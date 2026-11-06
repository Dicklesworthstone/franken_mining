//! DESIGN: source-bound declared plant worksheets over the accounting owners (BD-177).
//! Exact nominal accounting and conditional Gaussian reconciliation remain separate modes.
//! Determinism: Exact for balance; DeterministicPerIsa for reconciliation.

mod network;
mod project;
mod recovery;
mod transfer;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_plant::{
    inventory::{self, BalanceScope, BalanceValue, MetalBalanceInput},
    reconciliation::{
        self, InventoryErrorModel, InventoryReconciliation, InventoryReconciliationOutcome,
        Kilograms, KilogramsSquared,
    },
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use crate::{Envelope, NextAction, escape_json, worksheet_input};

pub const BALANCE_USAGE: &str = "plant balance --input <worksheet.toml>";
pub const RECONCILE_USAGE: &str = "plant reconcile --input <worksheet.toml>";
pub const PROJECT_USAGE: &str = "plant project --input <worksheet.toml>";
pub const PROJECT_DOCS_JSON: &str = project::ROBOT_DOCS_JSON;
pub const TRANSFER_USAGE: &str = "plant transfer --input <worksheet.toml>";
pub const TRANSFER_DOCS_JSON: &str = transfer::ROBOT_DOCS_JSON;
pub const NETWORK_USAGE: &str = "plant flowsheet --input <worksheet.toml>";
pub const NETWORK_DOCS_JSON: &str = network::ROBOT_DOCS_JSON;
pub const INFER_USAGE: &str = "plant infer --input <worksheet.toml>";
pub const INFERENCE_DOCS_JSON: &str = network::INFERENCE_ROBOT_DOCS_JSON;
pub const RECOVERY_USAGE: &str = "plant recovery --input <worksheet.toml>";
pub const RECOVERY_DOCS_JSON: &str = recovery::ROBOT_DOCS_JSON;
pub const SCHEMA: &str = "fmine.plant-inventory.v1";
const MAX_BYTES: u64 = 1024 * 1024;
const ROLES: [&str; 5] = ["opening", "feed", "product", "other_outflow", "closing"];
const NO_CLAIM: &str = "Calculation conditional on the submitted complete population and nominal observations. Submitted values and error policy remain Assumed, Internal and uncalibrated. No external source audit, sensor-bias identification, recovery estimate, P754 compliance, ledger commit or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-inventory.v1\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"element\",\"measurand\",\"basis\",\"start_ns\",\"end_ns\",\"complete\"],",
    "\"basis\":\"kg-total-element/all-phases/full-physical-boundary\",",
    "\"observation_rows\":[\"role\",\"kg\",\"start_ns\",\"end_ns\"],",
    "\"roles\":[\"opening\",\"feed\",\"product\",\"other_outflow\",\"closing\"],",
    "\"times\":\"Quoted integer nanoseconds on one declared time axis; stocks are exact endpoint instants and flows span the full reporting interval\",",
    "\"quantities\":\"Quoted exact decimals or n/d with a positive denominator; explicit zero totals required\",",
    "\"reconcile_fields\":[\"unbiased_gaussian_errors\",\"global_chi_square_limit\",\"closure_tolerance_kg\",\"error_model_basis\"],",
    "\"optional_reconcile_fields\":[\"normalized_adjustment_limit\"],",
    "\"covariance_rows\":[\"left\",\"right\",\"kg2\"],",
    "\"covariance\":\"All 15 unordered pairs of the five role names, including explicit zero cross terms; symmetric full covariance, positive definite; no independence inferred\",",
    "\"policy\":\"Gaussian flag must be true; test limits are finite and positive, kg closure tolerance is finite and nonnegative; all are caller-declared, not system confidence claims\",",
    "\"balance\":\"Exact closure only; error-model fields refuse instead of being ignored\",",
    "\"rejected\":\"Nonzero exit; original observations and available fitted diagnostics retained; admitted_claim is null\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; byte binding does not audit the worksheet\",",
    "\"examples\":[\"tests/plant/closed_inventory.toml\",\"tests/plant/measured_inventory.toml\"]}"
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Balance,
    Reconcile,
}
impl Mode {
    fn verb(self) -> &'static str {
        match self {
            Self::Balance => "plant balance",
            Self::Reconcile => "plant reconcile",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Inventory { mode: Mode, input: PathBuf },
    Project { input: PathBuf },
    Transfer { input: PathBuf },
    Network { input: PathBuf },
    Infer { input: PathBuf },
    Recovery { input: PathBuf },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.len() != 4
        || args[0] != "plant"
        || args[2] != "--input"
        || args[3].trim().is_empty()
        || args[3].starts_with("--")
    {
        return Err(format!(
            "{BALANCE_USAGE} | {RECONCILE_USAGE} | {PROJECT_USAGE} | {TRANSFER_USAGE} | {NETWORK_USAGE} | {INFER_USAGE} | {RECOVERY_USAGE}"
        ));
    }
    let input = PathBuf::from(&args[3]);
    match args[1].as_str() {
        "balance" => Ok(Command::Inventory {
            mode: Mode::Balance,
            input,
        }),
        "reconcile" => Ok(Command::Inventory {
            mode: Mode::Reconcile,
            input,
        }),
        "project" => Ok(Command::Project { input }),
        "transfer" => Ok(Command::Transfer { input }),
        "flowsheet" => Ok(Command::Network { input }),
        "infer" => Ok(Command::Infer { input }),
        "recovery" => Ok(Command::Recovery { input }),
        _ => Err(format!(
            "{BALANCE_USAGE} | {RECONCILE_USAGE} | {PROJECT_USAGE} | {TRANSFER_USAGE} | {NETWORK_USAGE} | {INFER_USAGE} | {RECOVERY_USAGE}"
        )),
    }
}

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
fn finite(value: (&str, usize)) -> Result<f64, Refusal> {
    let parsed = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite number"))?;
    if !parsed.is_finite() {
        return Err(bad(value.1, "expected a finite number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if parsed == 0.0 && mantissa.bytes().any(|byte| matches!(byte, b'1'..=b'9')) {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plant worksheet line {}: nonzero decimal underflows binary64",
                value.1
            ),
        ));
    }
    Ok(parsed)
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
fn role(value: (&str, usize)) -> Result<usize, Refusal> {
    ROLES
        .iter()
        .position(|r| *r == value.0)
        .ok_or_else(|| bad(value.1, format!("unknown observation role {}", value.0)))
}
fn declared<T>(
    value: T,
    tag: &str,
    metric: &str,
    additivity: AdditivityClass,
    source: &SourceAnchor,
    locator: String,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
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
struct Worksheet {
    inputs: Vec<Claim<MetalBalanceInput>>,
    model: Option<Claim<InventoryErrorModel>>,
    model_basis: Option<String>,
}
fn parse_worksheet(text: &str, source: &SourceAnchor, mode: Mode) -> Result<Worksheet, Refusal> {
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if document.rows.len() > 20 {
        return Err(bad(
            1,
            "at most five observations and fifteen covariance pairs are allowed",
        ));
    }
    let mut required = vec![
        "schema",
        "boundary",
        "element",
        "measurand",
        "basis",
        "start_ns",
        "end_ns",
        "complete",
    ];
    let optional = if mode == Mode::Reconcile {
        required.extend([
            "unbiased_gaussian_errors",
            "global_chi_square_limit",
            "closure_tolerance_kg",
            "error_model_basis",
        ]);
        vec!["normalized_adjustment_limit"]
    } else {
        vec![]
    };
    let top = fields(&document.top, &required, &optional, 1)?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "basis")?.0 != inventory::MASS_BASIS {
        return Err(bad(
            get(&top, "basis")?.1,
            format!("basis must be {}", inventory::MASS_BASIS),
        ));
    }
    if get(&top, "complete")?.0 != "true" {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "declare complete = \"true\"; missing streams are never assigned zero",
        ));
    }
    let scope = BalanceScope {
        boundary: get(&top, "boundary")?.0.into(),
        element: get(&top, "element")?.0.into(),
        measurand: get(&top, "measurand")?.0.into(),
        interval: ValidTime {
            start_ns: integer(get(&top, "start_ns")?)?,
            end_ns: integer(get(&top, "end_ns")?)?,
        },
    };
    let population = MetalBalanceInput {
        scope: scope.clone(),
        physical_time: scope.interval,
        value: BalanceValue::Population { complete: true },
    };
    let tag = population.canonical_tag();
    let mut inputs = vec![declared(
        population,
        &tag,
        inventory::INPUT_METRIC,
        AdditivityClass::NonAdditive,
        source,
        format!(
            "scope and complete declaration, line {}",
            get(&top, "complete")?.1
        ),
    )];
    let mut observations = BTreeMap::new();
    let mut covariance = [None; 25];
    for row in &document.rows {
        match row.table.as_str() {
            "observation" => {
                let values = fields(
                    &row.fields,
                    &["role", "kg", "start_ns", "end_ns"],
                    &[],
                    row.line,
                )?;
                let index = role(get(&values, "role")?)?;
                let q = quantity(get(&values, "kg")?)?;
                let value = match index {
                    0 => BalanceValue::OpeningInventory(q),
                    1 => BalanceValue::Feed(q),
                    2 => BalanceValue::Product(q),
                    3 => BalanceValue::OtherOutflow(q),
                    _ => BalanceValue::ClosingInventory(q),
                };
                let input = MetalBalanceInput {
                    scope: scope.clone(),
                    value,
                    physical_time: ValidTime {
                        start_ns: integer(get(&values, "start_ns")?)?,
                        end_ns: integer(get(&values, "end_ns")?)?,
                    },
                };
                let tag = input.canonical_tag();
                let claim = declared(
                    input,
                    &tag,
                    inventory::INPUT_METRIC,
                    value.additivity(),
                    source,
                    format!(
                        "observation {}: kg line {}; support lines {} and {}",
                        ROLES[index],
                        get(&values, "kg")?.1,
                        get(&values, "start_ns")?.1,
                        get(&values, "end_ns")?.1
                    ),
                );
                if observations.insert(index, claim).is_some() {
                    return Err(bad(row.line, "duplicate observation role"));
                }
            }
            "covariance" if mode == Mode::Reconcile => {
                let values = fields(&row.fields, &["left", "right", "kg2"], &[], row.line)?;
                let i = role(get(&values, "left")?)?;
                let j = role(get(&values, "right")?)?;
                if covariance[i * 5 + j].is_some() {
                    return Err(bad(
                        row.line,
                        "duplicate covariance pair (including reversed pairs)",
                    ));
                }
                let value = finite(get(&values, "kg2")?)?;
                covariance[i * 5 + j] = Some(value);
                covariance[j * 5 + i] = Some(value);
            }
            _ => {
                return Err(bad(
                    row.line,
                    format!("unsupported table {} for {}", row.table, mode.verb()),
                ));
            }
        }
    }
    if observations.len() != 5 {
        return Err(bad(
            1,
            "all five observation roles, including explicit zero totals, are required",
        ));
    }
    inputs.extend(observations.into_values());
    let mut model_basis = None;
    let model = if mode == Mode::Reconcile {
        let mut matrix = [KilogramsSquared::new(0.0); 25];
        for (i, value) in covariance.into_iter().enumerate() {
            matrix[i] = KilogramsSquared::new(value.ok_or_else(|| {
                bad(
                    1,
                    "all 15 covariance pairs are required, including explicit zero cross terms",
                )
            })?);
        }
        if get(&top, "unbiased_gaussian_errors")?.0 != "true" {
            return Err(bad(
                1,
                "reconciliation requires an explicit unbiased Gaussian error assumption",
            ));
        }
        let model = InventoryErrorModel {
            scope,
            observation_ids: core::array::from_fn(|i| inputs[i + 1].claim_id),
            covariance: matrix,
            unbiased_gaussian_errors: true,
            global_chi_square_limit: finite(get(&top, "global_chi_square_limit")?)?,
            normalized_adjustment_limit: top
                .get("normalized_adjustment_limit")
                .copied()
                .map(finite)
                .transpose()?,
            closure_tolerance_kg: Kilograms::new(finite(get(&top, "closure_tolerance_kg")?)?),
        };
        let tag = model.canonical_tag();
        let basis = get(&top, "error_model_basis")?;
        model_basis = Some(basis.0.into());
        Some(declared(
            model,
            &tag,
            reconciliation::ERROR_MODEL_METRIC,
            AdditivityClass::NonAdditive,
            source,
            format!(
                "error_model_basis line {}; all covariance rows and declared rejection limits",
                basis.1
            ),
        ))
    } else {
        None
    };
    Ok(Worksheet {
        inputs,
        model,
        model_basis,
    })
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}
fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn numbers(values: impl IntoIterator<Item = f64>) -> String {
    format!(
        "[{}]",
        values.into_iter().map(number).collect::<Vec<_>>().join(",")
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
fn observation_json(input: &Claim<MetalBalanceInput>) -> String {
    let (role, q) = match input.value.value {
        BalanceValue::Population { .. } => return claim_json(input),
        BalanceValue::OpeningInventory(q) => ("opening", q),
        BalanceValue::Feed(q) => ("feed", q),
        BalanceValue::Product(q) => ("product", q),
        BalanceValue::OtherOutflow(q) => ("other_outflow", q),
        BalanceValue::ClosingInventory(q) => ("closing", q),
    };
    format!(
        "{{\"role\":{},\"kg\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"claim\":{}}}",
        quoted(role),
        exact(q),
        input.value.physical_time.start_ns,
        input.value.physical_time.end_ns,
        claim_json(input)
    )
}
fn diagnostics(value: &InventoryReconciliation) -> String {
    let normalized = value
        .normalized_adjustments
        .iter()
        .map(|v| v.map_or_else(|| "null".into(), number))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"role_order\":[\"opening\",\"feed\",\"product\",\"other_outflow\",\"closing\"],",
            "\"observed_kg\":{},\"estimates_kg\":{},\"adjustments_kg\":{},\"posterior_covariance_kg2\":{},",
            "\"normalized_adjustments\":[{}],\"raw_residual_exact_kg\":{},\"raw_residual_kg\":{},",
            "\"reconciled_residual_kg\":{},\"chi_square\":{},\"degrees_of_freedom\":{}}}"
        ),
        numbers(value.observed_kg.map(|v| v.value())),
        numbers(value.estimates_kg.map(|v| v.value())),
        numbers(value.adjustments_kg.map(|v| v.value())),
        numbers(value.posterior_covariance.map(|v| v.value())),
        normalized,
        exact(value.raw_residual_exact_kg),
        number(value.raw_residual_kg.value()),
        number(value.reconciled_residual_kg.value()),
        number(value.chi_square),
        value.degrees_of_freedom
    )
}
fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect plant worksheet units, times, complete roles and covariance requirements",
    )]
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let (mode, input) = match command {
        Command::Inventory { mode, input } => (*mode, input),
        Command::Project { input } => return project::execute(input, root),
        Command::Transfer { input } => return transfer::execute(input, root),
        Command::Network { input } => return network::execute(input, root),
        Command::Infer { input } => return network::execute_unmeasured(input, root),
        Command::Recovery { input } => return recovery::execute(input, root),
    };
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(input), "plant worksheet", MAX_BYTES)?;
        let worksheet = parse_worksheet(&text, &source, mode)?;
        let scope = &worksheet.inputs[0].value.scope;
        let scope_json = format!(
            "{{\"boundary\":{},\"element\":{},\"measurand\":{},\"basis\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\"}}",
            quoted(&scope.boundary),
            quoted(&scope.element),
            quoted(&scope.measurand),
            quoted(inventory::MASS_BASIS),
            scope.interval.start_ns,
            scope.interval.end_ns
        );
        let inputs_json = worksheet.inputs[1..]
            .iter()
            .map(observation_json)
            .collect::<Vec<_>>()
            .join(",");
        let model_json = worksheet.model.as_ref().map_or_else(|| "null".into(), |model| format!(
            "{{\"claim\":{},\"basis\":{},\"covariance_kg2\":{},\"unbiased_gaussian_errors\":true,\"global_chi_square_limit\":{},\"normalized_adjustment_limit\":{},\"closure_tolerance_kg\":{}}}",
            claim_json(model), quoted(worksheet.model_basis.as_deref().unwrap_or("")), numbers(model.value.covariance.map(|v| v.value())),
            number(model.value.global_chi_square_limit), model.value.normalized_adjustment_limit.map_or_else(|| "null".into(), number), number(model.value.closure_tolerance_kg.value())));
        let (mut envelope, admitted, digest, result) = match mode {
            Mode::Balance => match inventory::account_interval(&worksheet.inputs) {
                Ok(receipt) => {
                    let s = receipt.summary();
                    let summary = format!(
                        "{{\"opening_inventory_kg\":{},\"feed_kg\":{},\"product_kg\":{},\"other_outflow_kg\":{},\"closing_inventory_kg\":{},\"inventory_change_kg\":{},\"residual_kg\":{},\"product_to_feed_ratio\":{},\"ratio_is_recovery\":false}}",
                        exact(s.opening_inventory_kg),
                        exact(s.feed_kg),
                        exact(s.product_kg),
                        exact(s.other_outflow_kg),
                        exact(s.closing_inventory_kg),
                        exact(s.inventory_change_kg),
                        exact(s.residual_kg),
                        s.product_to_feed_ratio.map_or_else(|| "null".into(), exact)
                    );
                    (
                        Envelope::ok(mode.verb(), String::new(), vec![]),
                        claim_json(receipt.claim()),
                        quoted(receipt.digest()),
                        summary,
                    )
                }
                Err(refusal) => (
                    Envelope::refused(mode.verb(), refusal, actions()),
                    "null".into(),
                    "null".into(),
                    "null".into(),
                ),
            },
            Mode::Reconcile => {
                let model = worksheet
                    .model
                    .as_ref()
                    .ok_or_else(|| bad(1, "error model required"))?;
                match reconciliation::reconcile_inventory(&worksheet.inputs, model) {
                    Ok(InventoryReconciliationOutcome::Admitted(receipt)) => (
                        Envelope::ok(mode.verb(), String::new(), vec![]),
                        claim_json(receipt.claim()),
                        quoted(receipt.digest()),
                        diagnostics(&receipt.claim().value),
                    ),
                    Ok(InventoryReconciliationOutcome::Rejected(report)) => (
                        Envelope::refused(mode.verb(), report.refusal().clone(), actions()),
                        "null".into(),
                        quoted(report.digest()),
                        diagnostics(report.diagnostics()),
                    ),
                    Err(refusal) => (
                        Envelope::refused(mode.verb(), refusal, actions()),
                        "null".into(),
                        "null".into(),
                        "null".into(),
                    ),
                }
            }
        };
        envelope.payload_json = format!(
            "{{\"schema\":\"fmine.plant-result.v1\",\"mode\":{},\"source\":{{\"path\":{},\"sha256\":{},\"bytes\":{}}},\"scope\":{},\"population\":{},\"observations\":[{}],\"error_model\":{},\"admitted_claim\":{},\"receipt_digest\":{},\"result\":{}}}",
            quoted(match mode {
                Mode::Balance => "exact_nominal_balance",
                Mode::Reconcile => "conditional_gaussian_reconciliation",
            }),
            quoted(&source.artifact_path),
            quoted(&Digest32(source.artifact_sha256).to_hex()),
            text.len(),
            scope_json,
            claim_json(&worksheet.inputs[0]),
            inputs_json,
            model_json,
            admitted,
            digest,
            result
        );
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused(mode.verb(), refusal, actions()));
    envelope.no_claim = Some(NO_CLAIM);
    envelope.warnings.push("Worksheet observations, completeness and any error policy are Assumed, Internal and uncalibrated; exact byte binding is not independent source verification.".into());
    envelope
}
