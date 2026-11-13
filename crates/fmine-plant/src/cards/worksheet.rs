//! DESIGN: source-bound recovery worksheets over the existing card owner (BD-176/287).
//! Every layer and processing-support declaration remains Assumed, Internal and uncalibrated.
//! Determinism: DeterministicPerIsa

use crate::cards::{
    self, CardVersion, InputRequirement, InputUnit, InputValue, OperationalFactor, OutputScale,
    RecoveryCorrection, RecoveryInputs, RecoveryModel, RecoveryScope, RouteAssumption,
    TestWorkCard,
    ensemble::{self, RecoveryPopulation, RecoveryScenario},
};
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlDoc, TomlField, TomlRow, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;
use std::collections::BTreeMap;

pub const SCHEMA: &str = "fmine.plant-recovery.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Nominal recovery conditional on the submitted formula, validity box, versioned layers, route and processing-support inputs. All declarations remain Assumed, Internal and uncalibrated. No source qualification, sample-to-blend transfer, uncertainty propagation, forecast registration, empirical calibration, P754 compliance or operating authorization is supplied.";
const ENSEMBLE_NO_CLAIM: &str = "Nominal recovery and optional joint-scenario propagation conditional on the submitted formula, validity box, versioned layers, route and processing-support inputs. All declarations, scenario membership and weights remain Assumed, Internal and uncalibrated. No source qualification, sample-to-blend transfer, inferred distribution or independence, forecast registration, empirical calibration, P754 compliance or operating authorization is supplied.";

pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-recovery.v1\",",
    "\"command\":\"plant recovery --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; all values quoted; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"circuit\",\"material_domain\",\"dry_basis\",\"element\",\"processing_support\",\"valid_start_ns\",\"valid_end_ns\",\"start_ns\",\"end_ns\"],",
    "\"processing_support\":\"Must be true: inputs already describe one simultaneous processing blend in the declared circuit/domain/dry basis; no block or sample aggregation is inferred\",",
    "\"times\":\"Quoted signed integer nanoseconds on one declared axis; positive card validity and processing intervals, with the processing interval inside the card validity\",",
    "\"test_work_rows\":[\"id\",\"revision\",\"formula\",\"output_scale\"],",
    "\"output_scale\":[\"fraction\",\"percent\"],",
    "\"formula\":\"ASCII arithmetic, parentheses, exp/EXP, ln/LN and integer powers from -16 to 16; at most 4096 bytes, 512 operations and 32 nesting levels; coordinates are used in their declared units\",",
    "\"requirement_rows\":[\"name\",\"measurand\",\"unit\",\"lower\",\"upper\",\"lower_inclusive\",\"upper_inclusive\"],",
    "\"input_rows\":[\"name\",\"measurand\",\"unit\",\"value\"],",
    "\"optional_input_fields\":[\"scenario\"],",
    "\"optional_scenario_rows\":[\"id\",\"weight\"],",
    "\"scenarios\":\"At most 256 named joint scenarios; strictly positive exact weights must sum to one. Each scenario supplies every coordinate through input rows with its scenario id, on the same scope and interval as nominal inputs. Missing, duplicate, undeclared or out-of-domain members refuse the entire calculation; no dropping, normalization or independent resampling\",",
    "\"ensemble\":\"Evaluate the full versioned formula separately for each member; emit weighted mean, population variance, inverse-CDF p10/p50/p90 without interpolation, exact cap probability and every member receipt. Scenario probabilities are declared and uncalibrated\",",
    "\"units\":[\"g_per_dry_t\",\"oz_per_dry_st\",\"dry_mass_percent\",\"dry_mass_fraction\",\"ppm_dry_mass\",\"spike_test_percent\",\"micrometres\",\"hours\"],",
    "\"coordinates\":\"At most 32 unique requirements and exactly their matching inputs; each input repeats the exact unit and measurand; no implicit conversions or extrapolation\",",
    "\"operation_rows\":[\"id\",\"revision\",\"multiplier\"],",
    "\"operations\":\"Zero through eight nonnegative multipliers, applied in worksheet order; duplicate id/revision pairs refuse\",",
    "\"correction_rows\":[\"id\",\"revision\",\"additive_fraction\"],",
    "\"optional_correction_fields\":[\"upper_cap_fraction\"],",
    "\"correction\":\"Signed additive fraction, so 0.02 is two percentage points; optional explicit cap in [0,1], with no implicit clipping\",",
    "\"route_rows\":[\"id\",\"revision\",\"route\"],",
    "\"layers\":\"Exactly one test_work, correction and route; every operation, correction and route binds the single test-work Claim\",",
    "\"quantities\":\"Exact plain decimals or n/d with a positive denominator; original numeric tokens are retained; bounds and fractions are checked before numerical evaluation\",",
    "\"output\":\"Original versioned layers, formula, exact bounds, source Claims, inputs, each intermediate fraction, explicit cap status and receipt digest; a kernel refusal retains parsed declarations with null result and admitted_claim\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; nominal evaluation does not establish source accuracy, applicability, forecast calibration or operating permission\",",
    "\"example\":\"tests/plant/recovery_card.toml\"}"
);

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

fn bad(line: usize, why: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("plant worksheet line {line}: {}", why.as_ref()),
    )
}

fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>, Refusal> {
    let mut result = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
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
                "values must be nonempty trimmed text of at most 4096 bytes",
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

fn integer(value: (&str, usize)) -> Result<i64, Refusal> {
    value.0.parse().map_err(|_| {
        bad(
            value.1,
            "expected a signed 64-bit integer time in nanoseconds",
        )
    })
}

fn quantity(value: (&str, usize)) -> Result<ExactQty, Refusal> {
    let fail = || {
        bad(
            value.1,
            "expected a plain exact decimal or n/d with a positive denominator",
        )
    };
    if let Some((n, d)) = value.0.split_once('/') {
        let n = n.parse::<i128>().map_err(|_| fail())?;
        let d = d.parse::<i128>().map_err(|_| fail())?;
        if d <= 0 {
            return Err(fail());
        }
        return ExactQty::new(n, d).map_err(|_| fail());
    }
    let digits = value.0.strip_prefix('-').unwrap_or(value.0);
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fractional.is_empty())
    {
        return Err(fail());
    }
    ExactQty::from_decimal_str(value.0).map_err(|_| fail())
}

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}

fn claim_json<T>(claim: &Claim<T>) -> String {
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                quoted(&a.artifact_path),
                quoted(&Digest32(a.artifact_sha256).to_hex()),
                quoted(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"metric\":{},\"evidence\":{},\"state\":{},\"confidentiality\":\"Internal\",\"calibration\":\"uncalibrated\",\"parents\":[{}],\"sources\":[{anchors}]}}",
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
        quoted(&format!("{:?}", claim.evidence)),
        quoted(&format!("{:?}", claim.state)),
        claim
            .parents
            .iter()
            .map(|id| quoted(&id.to_string()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

struct Worksheet {
    test_work: Claim<TestWorkCard>,
    operations: Vec<Claim<OperationalFactor>>,
    correction: Claim<RecoveryCorrection>,
    route: Claim<RouteAssumption>,
    inputs: Claim<RecoveryInputs>,
    population: Option<(Claim<RecoveryPopulation>, String)>,
    model_json: String,
    inputs_json: String,
}

fn single<'a>(document: &'a TomlDoc, table: &str) -> Result<&'a TomlRow, Refusal> {
    let mut rows = document.rows.iter().filter(|row| row.table == table);
    let row = rows
        .next()
        .ok_or_else(|| bad(1, format!("exactly one [[{table}]] row is required")))?;
    if let Some(duplicate) = rows.next() {
        return Err(bad(duplicate.line, format!("duplicate [[{table}]] row")));
    }
    Ok(row)
}

fn boolean(value: (&str, usize)) -> Result<bool, Refusal> {
    match value.0 {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad(value.1, "expected quoted true or false")),
    }
}

fn input_unit(value: (&str, usize)) -> Result<InputUnit, Refusal> {
    match value.0 {
        "g_per_dry_t" => Ok(InputUnit::GramsPerDryMetricTonne),
        "oz_per_dry_st" => Ok(InputUnit::TroyOuncesPerDryShortTon),
        "dry_mass_percent" => Ok(InputUnit::DryMassPercent),
        "dry_mass_fraction" => Ok(InputUnit::DryMassFraction),
        "ppm_dry_mass" => Ok(InputUnit::PartsPerMillionDryMass),
        "spike_test_percent" => Ok(InputUnit::SpikeTestPercent),
        "micrometres" => Ok(InputUnit::Micrometres),
        "hours" => Ok(InputUnit::Hours),
        _ => Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "plant recovery line {}: unsupported coordinate unit {}",
                value.1, value.0
            ),
        )),
    }
}

fn version(values: &Fields<'_>) -> Result<CardVersion, Refusal> {
    Ok(CardVersion {
        id: get(values, "id")?.0.into(),
        revision: get(values, "revision")?.0.into(),
    })
}

fn version_json(value: &CardVersion) -> String {
    format!(
        "{{\"id\":{},\"revision\":{}}}",
        quoted(&value.id),
        quoted(&value.revision)
    )
}

fn interval(values: &Fields<'_>, start: &str, end: &str) -> Result<ValidTime, Refusal> {
    Ok(ValidTime {
        start_ns: integer(get(values, start)?)?,
        end_ns: integer(get(values, end)?)?,
    })
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
            locator,
        )],
    )
}

fn parse(text: &str, source: &SourceAnchor) -> Result<Worksheet, Refusal> {
    let document = parse_subset(text).map_err(|error| bad(error.line, error.reason))?;
    if document.rows.len()
        > (ensemble::MAX_SCENARIOS + 2) * cards::MAX_INPUTS
            + ensemble::MAX_SCENARIOS
            + cards::MAX_OPERATIONAL_FACTORS
            + 3
    {
        return Err(bad(1, "recovery worksheet exceeds the bounded row count"));
    }
    for row in &document.rows {
        if !matches!(
            row.table.as_str(),
            "test_work"
                | "requirement"
                | "input"
                | "scenario"
                | "operation"
                | "correction"
                | "route"
        ) {
            return Err(bad(row.line, format!("unsupported table {}", row.table)));
        }
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "circuit",
            "material_domain",
            "dry_basis",
            "element",
            "processing_support",
            "valid_start_ns",
            "valid_end_ns",
            "start_ns",
            "end_ns",
        ],
        &[],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if !boolean(get(&top, "processing_support")?)? {
        return Err(Refusal::new(
            RefusalCode::SupportMismatch102,
            "declare processing_support = \"true\" only for simultaneous processing-blend inputs; block and sample support cannot substitute",
        ));
    }
    let scope = RecoveryScope {
        circuit: get(&top, "circuit")?.0.into(),
        material_domain: get(&top, "material_domain")?.0.into(),
        dry_basis: get(&top, "dry_basis")?.0.into(),
        element: get(&top, "element")?.0.into(),
    };
    let mut requirements = BTreeMap::new();
    let mut requirement_json = BTreeMap::new();
    for row in document.rows_of("requirement") {
        let values = fields(
            &row.fields,
            &[
                "name",
                "measurand",
                "unit",
                "lower",
                "upper",
                "lower_inclusive",
                "upper_inclusive",
            ],
            &[],
            row.line,
        )?;
        let name = get(&values, "name")?.0;
        let spec = InputRequirement {
            measurand: get(&values, "measurand")?.0.into(),
            unit: input_unit(get(&values, "unit")?)?,
            lower: quantity(get(&values, "lower")?)?,
            upper: quantity(get(&values, "upper")?)?,
            lower_inclusive: boolean(get(&values, "lower_inclusive")?)?,
            upper_inclusive: boolean(get(&values, "upper_inclusive")?)?,
        };
        requirement_json.insert(name.to_string(), format!(
            "{{\"name\":{},\"measurand\":{},\"unit\":{},\"raw_lower\":{},\"raw_upper\":{},\"lower\":{},\"upper\":{},\"lower_inclusive\":{},\"upper_inclusive\":{}}}",
            quoted(name), quoted(&spec.measurand), quoted(get(&values, "unit")?.0),
            quoted(get(&values, "lower")?.0), quoted(get(&values, "upper")?.0),
            exact(spec.lower), exact(spec.upper), spec.lower_inclusive, spec.upper_inclusive,
        ));
        if requirements.insert(name.to_string(), spec).is_some() {
            return Err(bad(row.line, format!("duplicate requirement {name}")));
        }
    }
    let row = single(&document, "test_work")?;
    let values = fields(
        &row.fields,
        &["id", "revision", "formula", "output_scale"],
        &[],
        row.line,
    )?;
    let output_scale = match get(&values, "output_scale")? {
        ("fraction", _) => OutputScale::Fraction,
        ("percent", _) => OutputScale::Percent,
        (_, line) => return Err(bad(line, "output_scale must be fraction or percent")),
    };
    let card = TestWorkCard {
        version: version(&values)?,
        scope: scope.clone(),
        valid_time: interval(&top, "valid_start_ns", "valid_end_ns")?,
        input_units: requirements,
        formula: get(&values, "formula")?.0.into(),
        output_scale,
    };
    let tag = card.canonical_tag();
    let test_work = source_claim(
        card,
        &tag,
        cards::TEST_WORK_METRIC,
        source,
        format!(
            "test_work row line {}; top-level scope and validity; all requirement rows",
            row.line,
        ),
    );
    let test_work_json = format!(
        "{{\"version\":{},\"valid_start_ns\":\"{}\",\"valid_end_ns\":\"{}\",\"formula\":{},\"output_scale\":{},\"requirements\":[{}],\"claim\":{}}}",
        version_json(&test_work.value.version),
        test_work.value.valid_time.start_ns,
        test_work.value.valid_time.end_ns,
        quoted(&test_work.value.formula),
        quoted(get(&values, "output_scale")?.0),
        requirement_json.into_values().collect::<Vec<_>>().join(","),
        claim_json(&test_work),
    );
    let mut operations = Vec::new();
    let mut operation_json = Vec::new();
    for row in document.rows_of("operation") {
        let values = fields(
            &row.fields,
            &["id", "revision", "multiplier"],
            &[],
            row.line,
        )?;
        let factor = OperationalFactor {
            version: version(&values)?,
            test_work: test_work.claim_id,
            multiplier: quantity(get(&values, "multiplier")?)?,
        };
        let tag = factor.canonical_tag();
        let factor = source_claim(
            factor,
            &tag,
            cards::OPERATION_METRIC,
            source,
            format!(
                "operation row line {}; targets the worksheet test_work",
                row.line
            ),
        );
        operation_json.push(format!(
            "{{\"version\":{},\"test_work_claim_id\":{},\"raw_multiplier\":{},\"multiplier\":{},\"claim\":{}}}",
            version_json(&factor.value.version), quoted(&test_work.claim_id.to_string()),
            quoted(get(&values, "multiplier")?.0), exact(factor.value.multiplier), claim_json(&factor),
        ));
        operations.push(factor);
    }
    let row = single(&document, "correction")?;
    let values = fields(
        &row.fields,
        &["id", "revision", "additive_fraction"],
        &["upper_cap_fraction"],
        row.line,
    )?;
    let correction = RecoveryCorrection {
        version: version(&values)?,
        test_work: test_work.claim_id,
        additive_fraction: quantity(get(&values, "additive_fraction")?)?,
        upper_cap_fraction: values
            .get("upper_cap_fraction")
            .copied()
            .map(quantity)
            .transpose()?,
    };
    let tag = correction.canonical_tag();
    let correction = source_claim(
        correction,
        &tag,
        cards::CORRECTION_METRIC,
        source,
        format!(
            "correction row line {}; targets the worksheet test_work",
            row.line
        ),
    );
    let correction_json = format!(
        "{{\"version\":{},\"test_work_claim_id\":{},\"raw_additive_fraction\":{},\"additive_fraction\":{},\"raw_upper_cap_fraction\":{},\"upper_cap_fraction\":{},\"claim\":{}}}",
        version_json(&correction.value.version),
        quoted(&test_work.claim_id.to_string()),
        quoted(get(&values, "additive_fraction")?.0),
        exact(correction.value.additive_fraction),
        values
            .get("upper_cap_fraction")
            .map_or_else(|| "null".into(), |value| quoted(value.0)),
        correction
            .value
            .upper_cap_fraction
            .map_or_else(|| "null".into(), exact),
        claim_json(&correction),
    );
    let row = single(&document, "route")?;
    let values = fields(&row.fields, &["id", "revision", "route"], &[], row.line)?;
    let route = RouteAssumption {
        version: version(&values)?,
        test_work: test_work.claim_id,
        route: get(&values, "route")?.0.into(),
    };
    let tag = route.canonical_tag();
    let route = source_claim(
        route,
        &tag,
        cards::ROUTE_METRIC,
        source,
        format!(
            "route row line {}; targets the worksheet test_work",
            row.line
        ),
    );
    let route_json = format!(
        "{{\"version\":{},\"test_work_claim_id\":{},\"route\":{},\"claim\":{}}}",
        version_json(&route.value.version),
        quoted(&test_work.claim_id.to_string()),
        quoted(&route.value.route),
        claim_json(&route),
    );
    let mut groups = BTreeMap::<_, (BTreeMap<String, InputValue>, BTreeMap<String, String>)>::new();
    for row in document.rows_of("input") {
        let values = fields(
            &row.fields,
            &["name", "measurand", "unit", "value"],
            &["scenario"],
            row.line,
        )?;
        let scenario = values.get("scenario").map_or("", |value| value.0);
        if values.contains_key("scenario") && scenario.trim().is_empty() {
            return Err(bad(row.line, "input scenario id must be nonempty"));
        }
        let (inputs, input_json) = groups.entry(scenario).or_default();
        let name = get(&values, "name")?.0;
        let input = InputValue {
            measurand: get(&values, "measurand")?.0.into(),
            unit: input_unit(get(&values, "unit")?)?,
            value: quantity(get(&values, "value")?)?,
        };
        input_json.insert(
            name.to_string(),
            format!(
                "{{\"name\":{},\"measurand\":{},\"unit\":{},\"raw_value\":{},\"value\":{}}}",
                quoted(name),
                quoted(&input.measurand),
                quoted(get(&values, "unit")?.0),
                quoted(get(&values, "value")?.0),
                exact(input.value),
            ),
        );
        if inputs.insert(name.to_string(), input).is_some() {
            return Err(bad(row.line, format!("duplicate input {name}")));
        }
    }
    let interval = interval(&top, "start_ns", "end_ns")?;
    let mut input_set = |id: &str, locator: String| {
        let (values, input_json) = groups.remove(id).unwrap_or_default();
        let inputs = RecoveryInputs {
            scope: scope.clone(),
            interval,
            values,
        };
        let tag = inputs.canonical_tag();
        let inputs = source_claim(inputs, &tag, cards::INPUT_METRIC, source, locator);
        let json = format!(
            "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"values\":[{}],\"claim\":{}}}",
            interval.start_ns,
            interval.end_ns,
            input_json.into_values().collect::<Vec<_>>().join(","),
            claim_json(&inputs),
        );
        (inputs, json)
    };
    let (inputs, inputs_json) = input_set(
        "",
        "top-level scope, processing_support and processing interval; all input rows".into(),
    );
    let mut scenarios = BTreeMap::new();
    let mut scenario_json = BTreeMap::new();
    for row in document.rows_of("scenario") {
        let values = fields(&row.fields, &["id", "weight"], &[], row.line)?;
        let id = get(&values, "id")?.0;
        if id.trim().is_empty() || scenarios.contains_key(id) {
            return Err(bad(row.line, "scenario id must be nonempty and unique"));
        }
        let weight = quantity(get(&values, "weight")?)?;
        let (inputs, json) = input_set(
            id,
            format!(
                "scenario {id}; all matching input rows; top-level processing scope and interval"
            ),
        );
        scenario_json.insert(
            id,
            format!(
                "{{\"id\":{},\"raw_weight\":{},\"weight\":{},\"inputs\":{json}}}",
                quoted(id),
                quoted(get(&values, "weight")?.0),
                exact(weight),
            ),
        );
        scenarios.insert(
            id,
            RecoveryScenario {
                id: id.into(),
                weight,
                inputs,
            },
        );
    }
    if !groups.is_empty() {
        return Err(bad(1, "input refers to an undeclared scenario"));
    }
    let population = if scenarios.is_empty() {
        None
    } else {
        let population = RecoveryPopulation {
            scenarios: scenarios.into_values().collect(),
        };
        let tag = population.canonical_tag();
        let population = source_claim(
            population,
            &tag,
            ensemble::INPUT_METRIC,
            source,
            "all scenario rows and their joint inputs; exact declared weights".into(),
        );
        let json = format!(
            "{{\"claim\":{},\"scenarios\":[{}]}}",
            claim_json(&population),
            scenario_json.into_values().collect::<Vec<_>>().join(",")
        );
        Some((population, json))
    };
    Ok(Worksheet {
        test_work,
        operations,
        correction,
        route,
        inputs,
        population,
        model_json: format!(
            "{{\"test_work\":{test_work_json},\"operations\":[{}],\"correction\":{correction_json},\"route\":{route_json}}}",
            operation_json.join(","),
        ),
        inputs_json,
    })
}

/// Source-bound declarations and the complete conditional calculation. A kernel
/// refusal retains the parsed model and inputs, with no admitted result.
pub struct WorksheetResult {
    json: String,
    refusal: Option<Refusal>,
    no_claim: &'static str,
    ensemble_receipt: Option<ensemble::RecoveryEnsembleReceipt>,
}

impl WorksheetResult {
    pub fn to_json(&self) -> String {
        self.json.clone()
    }
    pub fn refusal(&self) -> Option<&Refusal> {
        self.refusal.as_ref()
    }
    pub fn no_claim(&self) -> &'static str {
        self.no_claim
    }
    /// Admitted immutable joint population for downstream processing calculations.
    /// A refused member or a nominal-only worksheet cannot supply an ensemble.
    pub fn ensemble_receipt(&self) -> Option<&ensemble::RecoveryEnsembleReceipt> {
        self.ensemble_receipt.as_ref()
    }
}

/// Admit the exact source bytes and evaluate the nominal card and any declared
/// joint scenarios. File IO and output envelopes remain with the caller.
pub fn evaluate(text: &str, source: &SourceAnchor) -> Result<WorksheetResult, Refusal> {
    if text.len() as u64 > MAX_BYTES {
        return Err(bad(1, "worksheet exceeds 1 MiB"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "plant recovery worksheet bytes differ from source SHA-256",
        ));
    }
    let worksheet = parse(text, source)?;
    let evaluation = RecoveryModel::compile(
        &worksheet.test_work,
        &worksheet.operations,
        &worksheet.correction,
        &worksheet.route,
    )
    .and_then(|model| {
        let nominal = model.evaluate(&worksheet.inputs)?;
        let ensemble = worksheet
            .population
            .as_ref()
            .map(|(population, _)| model.evaluate_ensemble(population))
            .transpose()?;
        Ok((nominal, ensemble))
    });
    let (refusal, admitted, digest, result, ensemble_receipt) = match evaluation {
        Ok((receipt, ensemble)) => (
            None,
            claim_json(receipt.claim()),
            quoted(receipt.digest()),
            receipt.result_json(),
            ensemble,
        ),
        Err(refusal) => (
            Some(refusal),
            "null".into(),
            "null".into(),
            "null".into(),
            None,
        ),
    };
    let ensemble = ensemble_receipt
        .as_ref()
        .map_or_else(|| "null".into(), |receipt| receipt.to_json());
    let scope = &worksheet.test_work.value.scope;
    let population = worksheet
        .population
        .as_ref()
        .map_or_else(String::new, |(_, declarations)| {
            format!(",\"population\":{declarations},\"ensemble\":{ensemble}")
        });
    let json = format!(
        "{{\"schema\":\"fmine.plant-recovery-result.v1\",\"mode\":\"nominal_recovery_card\",\"source\":{{\"path\":{},\"sha256\":{},\"bytes\":{}}},\"scope\":{{\"circuit\":{},\"material_domain\":{},\"dry_basis\":{},\"element\":{},\"processing_support\":true}},\"model\":{},\"inputs\":{},\"admitted_claim\":{admitted},\"receipt_digest\":{digest},\"result\":{result}{population}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        text.len(),
        quoted(&scope.circuit),
        quoted(&scope.material_domain),
        quoted(&scope.dry_basis),
        quoted(&scope.element),
        worksheet.model_json,
        worksheet.inputs_json,
    );
    let no_claim = if worksheet.population.is_some() {
        ENSEMBLE_NO_CLAIM
    } else {
        NO_CLAIM
    };
    Ok(WorksheetResult {
        json,
        refusal,
        no_claim,
        ensemble_receipt,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSHEET: &str = include_str!("../../../../tests/plant/recovery_card.toml");

    fn source(text: &str) -> SourceAnchor {
        SourceAnchor::new(
            "synthetic-recovery.toml",
            sha256(text.as_bytes()).0,
            "worksheet",
        )
    }

    #[test]
    fn direct_worksheet_admission_binds_source_bytes_and_enforces_the_size_limit() {
        let result = evaluate(WORKSHEET, &source(WORKSHEET)).unwrap();
        assert!(result.refusal().is_none());
        assert!(result.to_json().contains("\"recovery_fraction\":0.79"));
        assert_eq!(result.no_claim(), NO_CLAIM);
        assert_eq!(
            evaluate(WORKSHEET, &source("different source"))
                .err()
                .unwrap()
                .code,
            RefusalCode::DigestMismatch505.spec().code,
        );
        let oversized = " ".repeat(MAX_BYTES as usize + 1);
        assert_eq!(
            evaluate(&oversized, &source(WORKSHEET)).err().unwrap().code,
            RefusalCode::InputMissingOrUnreadable506.spec().code,
        );
    }

    #[test]
    fn kernel_refusal_retains_the_parsed_source_model_and_inputs() {
        let text = WORKSHEET.replace("70 + 2 * Au - S", "ln(-1)");
        let result = evaluate(&text, &source(&text)).unwrap();
        assert!(result.refusal().is_some());
        let json = result.to_json();
        for retained in ["\"formula\":\"ln(-1)\"", "\"inputs\":{", "\"sources\":[{"] {
            assert!(json.contains(retained), "{retained}");
        }
        for absent in ["admitted_claim", "receipt_digest", "result"] {
            assert!(json.contains(&format!("\"{absent}\":null")), "{absent}");
        }
    }
}
