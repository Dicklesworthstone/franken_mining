//! DESIGN: source-bound coupling of admitted grinding and recovery populations.
//! The worksheet declares the physical bridge and preserves every simultaneous state.
//! Determinism: DeterministicPerIsa

use super::{ProductionInputs, ProductionReceipt, ProductionScenario};
use crate::cards::{RecoveryScope, ensemble::RecoveryEnsembleReceipt};
use crate::grinding::{GrindingReceipt, GrindingScope};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;
use std::collections::BTreeMap;

pub const SCHEMA: &str = "fmine.plant-production.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Conditional production from explicitly coupled grinding, grade, exposure and recovery scenarios. Probabilities, physical scope, coupling and model applicability remain declared, Assumed, Internal and uncalibrated. Scheduled hours include downtime through the grinding utilization exactly once. No physical blending rule, circuit feedback, inventory timing, payable-metal conversion, calibrated forecast or operating authorization is established.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-production.v1\",",
    "\"command\":\"plant production --input <coupling.toml> --grinding <grinding.toml> --recovery <recovery.toml>\",",
    "\"input\":\"Three regular UTF-8 files, each at most 1 MiB; each exact source is SHA-256 bound before evaluation\",",
    "\"fields\":[\"schema\",\"boundary\",\"circuit\",\"blend\",\"material_domain\",\"dry_basis\",\"element\",\"start_ns\",\"end_ns\",\"grade_unit\",\"duration_unit\",\"coupling_basis\",\"grade_coordinate\",\"product_size_coordinate\"],",
    "\"scenario_rows\":[\"id\",\"weight\",\"grade_g_per_dry_t\",\"scheduled_hours\"],",
    "\"units\":\"grade_unit=g_per_dry_t; duration_unit=hours; outputs dry metric tonnes and elemental kg; no unit or ownership conversion is inferred\",",
    "\"population\":\"Every grinding, recovery and production member must have the same unique id and exact positive probability; populations must match completely, without resampling or normalization\",",
    "\"coupling\":\"The explicit basis links the named grinding blend to the named recovery material domain and dry state; circuit and physical interval must match. Recovery grade and P80 coordinates are required and checked against the production grade and grinding product size\",",
    "\"duration\":\"Scheduled hours lie between zero and the physical interval duration; utilization is already included in grinding throughput and is not multiplied again\",",
    "\"values\":\"Quoted trimmed strings; exact decimals or n/d with positive denominator for weights, grades and hours; signed integer nanoseconds for times\",",
    "\"output\":\"Retains three submitted worksheets, source identities, admitted parent receipts, every coupled row, typed distributions and Claim ancestry; invalid members refuse the complete calculation\",",
    "\"quantiles\":\"Inverse CDF under exact supplied weights, without interpolation; descriptive conditional distribution, not confidence or calibration\",",
    "\"examples\":[\"tests/plant/production_joint.toml\",\"tests/plant/grinding_joint.toml\",\"tests/plant/production_recovery.toml\"]}"
);

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!(
            "plant production worksheet line {line}: {}",
            detail.as_ref()
        ),
    )
}

fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    line: usize,
) -> Result<Fields<'a>, Refusal> {
    let mut result = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if value.is_empty()
            || value.trim() != value
            || value.len() > 4096
            || value.chars().any(char::is_control)
        {
            return Err(bad(
                field.line,
                "expected nonempty trimmed text within 4096 bytes",
            ));
        }
        result.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(result)
}

fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}

fn exact(value: (&str, usize)) -> Result<ExactQty, Refusal> {
    let invalid = || {
        bad(
            value.1,
            "expected an exact decimal or n/d with positive denominator",
        )
    };
    if let Some((n, d)) = value.0.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| invalid())?;
        let d = d.parse::<i128>().map_err(|_| invalid())?;
        if d <= 0 {
            return Err(invalid());
        }
        return ExactQty::new(n, d).map_err(|_| invalid());
    }
    let digits = value.0.strip_prefix('-').unwrap_or(value.0);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fraction.is_empty())
    {
        return Err(invalid());
    }
    ExactQty::from_decimal_str(value.0).map_err(|_| invalid())
}

fn time(value: (&str, usize)) -> Result<i64, Refusal> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected signed integer nanoseconds"))
}

/// Immutable source and production receipt; failed admission never constructs this value.
pub struct WorksheetResult {
    submitted: String,
    source: SourceAnchor,
    receipt: ProductionReceipt,
}

impl WorksheetResult {
    pub fn receipt(&self) -> &ProductionReceipt {
        &self.receipt
    }
    pub fn to_json(&self) -> String {
        super::render::worksheet_json(&self.submitted, &self.source, &self.receipt)
    }
    pub fn to_markdown(&self) -> String {
        super::render::worksheet_markdown(&self.receipt)
    }
}

/// Parse the coupling declaration and evaluate the complete paired populations.
/// Both model receipts must already have been admitted by their original owners.
pub fn evaluate(
    text: &str,
    source: &SourceAnchor,
    grinding: &GrindingReceipt,
    recovery: &RecoveryEnsembleReceipt,
) -> Result<WorksheetResult, Refusal> {
    if text.len() as u64 > MAX_BYTES {
        return Err(bad(1, "worksheet exceeds 1 MiB"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "plant production worksheet differs from source SHA-256",
        ));
    }
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if doc.rows.is_empty() || doc.rows.len() > 256 {
        return Err(bad(1, "requires 1..256 complete scenario rows"));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "boundary",
            "circuit",
            "blend",
            "material_domain",
            "dry_basis",
            "element",
            "start_ns",
            "end_ns",
            "grade_unit",
            "duration_unit",
            "coupling_basis",
            "grade_coordinate",
            "product_size_coordinate",
        ],
        1,
    )?;
    for (key, expected) in [
        ("schema", SCHEMA),
        ("grade_unit", "g_per_dry_t"),
        ("duration_unit", "hours"),
    ] {
        let value = get(&top, key)?;
        if value.0 != expected {
            return Err(bad(value.1, format!("{key} must be {expected}")));
        }
    }
    let interval = ValidTime {
        start_ns: time(get(&top, "start_ns")?)?,
        end_ns: time(get(&top, "end_ns")?)?,
    };
    let circuit = get(&top, "circuit")?.0.to_owned();
    let mut scenarios = Vec::with_capacity(doc.rows.len());
    for row in &doc.rows {
        if row.table != "scenario" {
            return Err(bad(row.line, format!("unsupported table {}", row.table)));
        }
        let values = fields(
            &row.fields,
            &["id", "weight", "grade_g_per_dry_t", "scheduled_hours"],
            row.line,
        )?;
        scenarios.push(ProductionScenario {
            id: get(&values, "id")?.0.to_owned(),
            weight: exact(get(&values, "weight")?)?,
            grade_g_per_dry_t: exact(get(&values, "grade_g_per_dry_t")?)?,
            scheduled_hours: exact(get(&values, "scheduled_hours")?)?,
        });
    }
    let value = ProductionInputs {
        grinding_scope: GrindingScope {
            boundary: get(&top, "boundary")?.0.to_owned(),
            circuit: circuit.clone(),
            blend: get(&top, "blend")?.0.to_owned(),
            interval,
        },
        recovery_scope: RecoveryScope {
            circuit,
            material_domain: get(&top, "material_domain")?.0.to_owned(),
            dry_basis: get(&top, "dry_basis")?.0.to_owned(),
            element: get(&top, "element")?.0.to_owned(),
        },
        coupling_basis: get(&top, "coupling_basis")?.0.to_owned(),
        grade_coordinate: get(&top, "grade_coordinate")?.0.to_owned(),
        product_size_coordinate: get(&top, "product_size_coordinate")?.0.to_owned(),
        scenarios,
    };
    let tag = value.canonical_tag();
    let inputs = Claim::new(
        super::INPUT_METRIC,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::internal(),
        vec![],
        vec![],
        vec![source.clone()],
    );
    let receipt = super::evaluate(grinding, recovery, &inputs)?;
    Ok(WorksheetResult {
        submitted: text.to_owned(),
        source: source.clone(),
        receipt,
    })
}
