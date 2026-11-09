//! DESIGN: strict source-bound Bond worksheet. All joint rows and declarations
//! are retained; a bad row refuses the complete calculation.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::Confidentiality;
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::EpistemicStatus;
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "fmine.plant-grinding.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Conditional Bond energy and power-limited throughput at the submitted processing-blend support. Complete simultaneous scenario rows and declared probabilities are preserved; these probabilities are uncalibrated. Inputs and validity limits remain Assumed and Internal. The declared work-index test basis is not independently qualified. No physical blending rule, circuit-efficiency correction, other bottleneck, measured capacity, parameter fitting, forecast registration or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-grinding.v1\",\"command\":\"plant grinding --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"circuit\",\"blend\",\"start_ns\",\"end_ns\",\"mass_basis\",\"energy_unit\",\"size_unit\",\"power_unit\",\"model_id\",\"model_version\",\"work_index_test_basis\",\"validity_basis\",\"complete_joint_population\",\"min_work_index\",\"max_work_index\",\"min_f80_microns\",\"max_f80_microns\",\"min_p80_microns\",\"max_p80_microns\",\"min_power_kw\",\"max_power_kw\",\"min_utilization\",\"max_utilization\"],",
    "\"units\":\"mass_basis=dry_metric_tonne, energy_unit=kWh, size_unit=micron, power_unit=kW; work index and energy use kWh/dry_metric_tonne; throughput uses dry_metric_tonne/hour; utilization is a fraction\",",
    "\"scenario_rows\":[\"id\",\"weight\",\"work_index\",\"f80_microns\",\"p80_microns\",\"power_kw\",\"utilization\"],",
    "\"population\":\"1..4096 unique complete simultaneous rows of one processing blend and interval; complete_joint_population=true; strictly positive exact weights sum exactly to 1; no resampling or independence inferred\",",
    "\"values\":\"All values quoted trimmed strings; weights are exact plain decimals or n/d with positive denominator; physical coordinates are finite decimal numbers, optional exponent; times are signed integer nanoseconds\",",
    "\"domain\":\"Positive Wi and sizes, F80>P80, nonnegative power and utilization in [0,1]; every coordinate must lie inside its declared closed validity bounds; zero power or utilization returns zero throughput with positive energy\",",
    "\"output\":\"Every source Claim and original worksheet, each scenario energy/throughput, weighted means and inverse-CDF p10/p50/p90 with exact probabilities; no partial admitted result after a refusal\",",
    "\"interpretation\":\"Scenario weights describe alternative states, not physical ore blending; mean throughput averages row throughputs, not hardness before inversion; quantiles are conditional, not confidence intervals\",",
    "\"examples\":[\"tests/plant/grinding_bond.toml\",\"tests/plant/grinding_joint.toml\"]}"
);

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;
fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    fail(
        RefusalCode::InputMissingOrUnreadable506,
        format!("worksheet line {line}: {}", detail.as_ref()),
    )
}
fn fields<'a>(input: &'a [TomlField], required: &[&str], line: usize) -> Result<Fields<'a>> {
    let mut out = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if !named(value) {
            return Err(bad(
                field.line,
                "values must be nonempty trimmed text of at most 4096 bytes",
            ));
        }
        out.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !out.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(out)
}
fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize)> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}
fn numeric(value: (&str, usize)) -> Result<f64> {
    let parsed = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite decimal number"))?;
    if !parsed.is_finite() {
        return Err(bad(value.1, "expected a finite decimal number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if parsed == 0.0 && mantissa.bytes().any(|b| matches!(b, b'1'..=b'9')) {
        return Err(numerical(format!(
            "worksheet line {}: nonzero decimal underflows binary64",
            value.1
        )));
    }
    Ok(if parsed == 0.0 { 0.0 } else { parsed })
}
fn integer(value: (&str, usize)) -> Result<i64> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected signed integer nanoseconds"))
}
fn probability(value: (&str, usize)) -> Result<ExactQty> {
    let invalid = || {
        bad(
            value.1,
            "weight must be an exact plain decimal or n/d with positive denominator",
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
fn source_claim<T>(
    value: T,
    tag: &str,
    metric: &str,
    source: &SourceAnchor,
    locator: String,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::internal(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            &source.artifact_path,
            source.artifact_sha256,
            format!("{}:{locator}", source.locator),
        )],
    )
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorksheetResult {
    submitted: String,
    receipt: GrindingReceipt,
}
impl WorksheetResult {
    pub fn receipt(&self) -> &GrindingReceipt {
        &self.receipt
    }
    pub fn digest(&self) -> &str {
        self.receipt.digest()
    }
    pub fn to_json(&self) -> String {
        super::render::worksheet_json(&self.submitted, &self.receipt)
    }
    pub fn to_markdown(&self) -> String {
        super::render::worksheet_markdown(&self.receipt)
    }
}

/// Verify exact submitted source bytes, parse the closed grammar and evaluate
/// the entire declared population. No files or clocks are read by this adapter.
pub fn evaluate(text: &str, source: &SourceAnchor) -> Result<WorksheetResult> {
    if text.len() as u64 > MAX_BYTES || !named(&source.artifact_path) || !named(&source.locator) {
        return Err(bad(1, "bounded worksheet and named source anchor required"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(fail(
            RefusalCode::DigestMismatch505,
            "worksheet bytes differ from source SHA-256",
        ));
    }
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if document.rows.is_empty() || document.rows.len() > MAX_SCENARIOS {
        return Err(bad(1, "requires 1..4096 scenario rows"));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "circuit",
            "blend",
            "start_ns",
            "end_ns",
            "mass_basis",
            "energy_unit",
            "size_unit",
            "power_unit",
            "model_id",
            "model_version",
            "work_index_test_basis",
            "validity_basis",
            "complete_joint_population",
            "min_work_index",
            "max_work_index",
            "min_f80_microns",
            "max_f80_microns",
            "min_p80_microns",
            "max_p80_microns",
            "min_power_kw",
            "max_power_kw",
            "min_utilization",
            "max_utilization",
        ],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    for (field, required) in [
        ("mass_basis", "dry_metric_tonne"),
        ("energy_unit", "kWh"),
        ("size_unit", "micron"),
        ("power_unit", "kW"),
    ] {
        let (actual, line) = get(&top, field)?;
        if actual != required {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                format!("line {line}: {field} must be {required}; no implicit conversion"),
            ));
        }
    }
    let scope = GrindingScope {
        boundary: get(&top, "boundary")?.0.into(),
        circuit: get(&top, "circuit")?.0.into(),
        blend: get(&top, "blend")?.0.into(),
        interval: ValidTime {
            start_ns: integer(get(&top, "start_ns")?)?,
            end_ns: integer(get(&top, "end_ns")?)?,
        },
    };
    let complete = get(&top, "complete_joint_population")?;
    if !matches!(complete.0, "true" | "false") {
        return Err(bad(
            complete.1,
            "complete_joint_population must be quoted true or false",
        ));
    }
    let model = BondModel {
        scope: scope.clone(),
        id: get(&top, "model_id")?.0.into(),
        version: get(&top, "model_version")?.0.into(),
        work_index_test_basis: get(&top, "work_index_test_basis")?.0.into(),
        validity_basis: get(&top, "validity_basis")?.0.into(),
        complete_joint_population: complete.0 == "true",
        work_index_domain: [
            KwhPerDryTonne::new(numeric(get(&top, "min_work_index")?)?),
            KwhPerDryTonne::new(numeric(get(&top, "max_work_index")?)?),
        ],
        feed_size_domain: [
            Micrometres::new(numeric(get(&top, "min_f80_microns")?)?),
            Micrometres::new(numeric(get(&top, "max_f80_microns")?)?),
        ],
        product_size_domain: [
            Micrometres::new(numeric(get(&top, "min_p80_microns")?)?),
            Micrometres::new(numeric(get(&top, "max_p80_microns")?)?),
        ],
        power_domain: [
            Kilowatts::new(numeric(get(&top, "min_power_kw")?)?),
            Kilowatts::new(numeric(get(&top, "max_power_kw")?)?),
        ],
        utilization_domain: [
            Fraction::new(numeric(get(&top, "min_utilization")?)?),
            Fraction::new(numeric(get(&top, "max_utilization")?)?),
        ],
    };
    let tag = model.canonical_tag();
    let model = source_claim(
        model,
        &tag,
        MODEL_METRIC,
        source,
        "top-level model, units, processing support and validity declarations".into(),
    );
    let mut scenarios = Vec::with_capacity(document.rows.len());
    for row in &document.rows {
        if row.table != "scenario" {
            return Err(bad(row.line, format!("unsupported table {}", row.table)));
        }
        let values = fields(
            &row.fields,
            &[
                "id",
                "weight",
                "work_index",
                "f80_microns",
                "p80_microns",
                "power_kw",
                "utilization",
            ],
            row.line,
        )?;
        let scenario = GrindingScenario {
            id: get(&values, "id")?.0.into(),
            scope: scope.clone(),
            weight: probability(get(&values, "weight")?)?,
            work_index: KwhPerDryTonne::new(numeric(get(&values, "work_index")?)?),
            feed_size: Micrometres::new(numeric(get(&values, "f80_microns")?)?),
            product_size: Micrometres::new(numeric(get(&values, "p80_microns")?)?),
            installed_power: Kilowatts::new(numeric(get(&values, "power_kw")?)?),
            utilization: Fraction::new(numeric(get(&values, "utilization")?)?),
        };
        let tag = scenario.canonical_tag();
        scenarios.push(source_claim(
            scenario,
            &tag,
            INPUT_METRIC,
            source,
            format!("scenario row at line {}", row.line),
        ));
    }
    let receipt = super::evaluate(&model, &scenarios)?;
    Ok(WorksheetResult {
        submitted: text.into(),
        receipt,
    })
}
