//! DESIGN: source-bound nominal recovery worksheets over the existing card owner (BD-176/287).
//! Every layer and processing-support declaration remains Assumed, Internal and uncalibrated.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;
use std::path::Path;

use fmine_claim::{AdditivityClass, Claim};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_plant::cards::{
    self, CardVersion, InputRequirement, InputUnit, InputValue, OperationalFactor, OutputScale,
    RecoveryCorrection, RecoveryInputs, RecoveryModel, RecoveryScope, RouteAssumption,
    TestWorkCard,
};
use fmine_registry::toml_subset::{TomlDoc, TomlRow, parse_subset};
use fmine_time::ValidTime;

use super::{
    Fields, bad, claim_json, declared, exact, fields, get, integer, number, numbers, quantity,
    quoted,
};
use crate::{Envelope, NextAction, worksheet_input};

const SCHEMA: &str = "fmine.plant-recovery.v1";
const MAX_BYTES: u64 = 1024 * 1024;
const NO_CLAIM: &str = "Nominal recovery conditional on the submitted formula, validity box, versioned layers, route and processing-support inputs. All declarations remain Assumed, Internal and uncalibrated. No source qualification, sample-to-blend transfer, uncertainty propagation, forecast registration, empirical calibration, P754 compliance or operating authorization is supplied.";

pub(super) const ROBOT_DOCS_JSON: &str = concat!(
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

struct Worksheet {
    test_work: Claim<TestWorkCard>,
    operations: Vec<Claim<OperationalFactor>>,
    correction: Claim<RecoveryCorrection>,
    route: Claim<RouteAssumption>,
    inputs: Claim<RecoveryInputs>,
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
    declared(
        value,
        tag,
        metric,
        AdditivityClass::NonAdditive,
        source,
        locator,
    )
}

fn parse(text: &str, source: &SourceAnchor) -> Result<Worksheet, Refusal> {
    let document = parse_subset(text).map_err(|error| bad(error.line, error.reason))?;
    if document.rows.len() > 2 * cards::MAX_INPUTS + cards::MAX_OPERATIONAL_FACTORS + 3 {
        return Err(bad(1, "recovery worksheet exceeds the bounded row count"));
    }
    for row in &document.rows {
        if !matches!(
            row.table.as_str(),
            "test_work" | "requirement" | "input" | "operation" | "correction" | "route"
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
    let mut inputs = BTreeMap::new();
    let mut input_json = BTreeMap::new();
    for row in document.rows_of("input") {
        let values = fields(
            &row.fields,
            &["name", "measurand", "unit", "value"],
            &[],
            row.line,
        )?;
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
    let inputs = RecoveryInputs {
        scope,
        interval: interval(&top, "start_ns", "end_ns")?,
        values: inputs,
    };
    let tag = inputs.canonical_tag();
    let inputs = source_claim(
        inputs,
        &tag,
        cards::INPUT_METRIC,
        source,
        "top-level scope, processing_support and processing interval; all input rows".into(),
    );
    let inputs_json = format!(
        "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"values\":[{}],\"claim\":{}}}",
        inputs.value.interval.start_ns,
        inputs.value.interval.end_ns,
        input_json.into_values().collect::<Vec<_>>().join(","),
        claim_json(&inputs),
    );
    Ok(Worksheet {
        test_work,
        operations,
        correction,
        route,
        inputs,
        model_json: format!(
            "{{\"test_work\":{test_work_json},\"operations\":[{}],\"correction\":{correction_json},\"route\":{route_json}}}",
            operation_json.join(","),
        ),
        inputs_json,
    })
}

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect declared recovery-card units, validity bounds and versioned layers",
    )]
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(input), "plant recovery worksheet", MAX_BYTES)?;
        let worksheet = parse(&text, &source)?;
        let evaluation = RecoveryModel::compile(
            &worksheet.test_work,
            &worksheet.operations,
            &worksheet.correction,
            &worksheet.route,
        )
        .and_then(|model| model.evaluate(&worksheet.inputs));
        let (mut envelope, admitted, digest, result) = match evaluation {
            Ok(receipt) => {
                let result = &receipt.claim().value;
                (
                    Envelope::ok("plant recovery", String::new(), vec![]),
                    claim_json(receipt.claim()),
                    quoted(receipt.digest()),
                    format!(
                        "{{\"test_work_fraction\":{},\"operational_fractions\":{},\"corrected_before_cap\":{},\"cap_applied\":{},\"recovery_fraction\":{},\"route\":{}}}",
                        number(result.test_work_fraction.value()),
                        numbers(
                            result
                                .operational_fractions
                                .iter()
                                .map(|value| value.value())
                        ),
                        number(result.corrected_before_cap.value()),
                        result.cap_applied,
                        number(result.recovery_fraction.value()),
                        quoted(&result.route),
                    ),
                )
            }
            Err(refusal) => (
                Envelope::refused("plant recovery", refusal, actions()),
                "null".into(),
                "null".into(),
                "null".into(),
            ),
        };
        let scope = &worksheet.test_work.value.scope;
        envelope.payload_json = format!(
            "{{\"schema\":\"fmine.plant-recovery-result.v1\",\"mode\":\"nominal_recovery_card\",\"source\":{{\"path\":{},\"sha256\":{},\"bytes\":{}}},\"scope\":{{\"circuit\":{},\"material_domain\":{},\"dry_basis\":{},\"element\":{},\"processing_support\":true}},\"model\":{},\"inputs\":{},\"admitted_claim\":{admitted},\"receipt_digest\":{digest},\"result\":{result}}}",
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
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant recovery", refusal, actions()));
    envelope.no_claim = Some(NO_CLAIM);
    envelope.warnings.push("Formula, applicability bounds, versioned layers, route and processing-support inputs are Assumed, Internal and uncalibrated; source-byte binding does not establish plant applicability or measurement accuracy.".into());
    envelope
}
