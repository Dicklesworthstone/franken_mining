//! DESIGN: closed, source-byte-bound grammar for conditional inventory trajectories.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_claim::{Confidentiality, Representation, SpatialSupport};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

pub const SCHEMA: &str = "fmine.plant-trajectory.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Conditional linear inventory reconciliation for one storage boundary and one total elemental measurand across declared contiguous periods. Only genuine observations enter the supplied full covariance; missing boundary stocks have no prior or invented measurement. Submitted observations, completeness and unbiased Gaussian error model remain Assumed, Internal and uncalibrated. Later surveys condition a new retrospective result, not an earlier decision forecast. No sensor-bias estimation, robust likelihood, stockpile mixing, leach dynamics, empirical calibration, P754 compliance, ledger commit or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-trajectory.v1\",\"command\":\"plant trajectory --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; exact bytes SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"element\",\"measurand\",\"basis\",\"start_ns\",\"end_ns\",\"complete\",\"error_model_basis\",\"unbiased_gaussian_errors\",\"global_chi_square_limit\",\"closure_tolerance_kg\"],",
    "\"optional_fields\":[\"normalized_adjustment_limit\"],",
    "\"period_rows\":[\"start_ns\",\"end_ns\",\"inflow_kg\",\"outflow_kg\"],",
    "\"periods\":\"2..32 positive intervals exactly tile the full scope; every inflow and outflow total is supplied, including zeros\",",
    "\"stocktake_rows\":[\"at_ns\",\"kg\"],\"unmeasured_stocktake_rows\":[\"at_ns\"],",
    "\"stocks\":\"Exactly one measured or explicitly unmeasured stock per period boundary; a shared endpoint is one coordinate; at least one measured stock anchors the trajectory\",",
    "\"covariance_rows\":[\"left\",\"right\",\"kg2\"],",
    "\"coordinate_keys\":\"stock/<instant>; inflow/<start>/<end>; outflow/<start>/<end>; nanoseconds use canonical signed integer spelling\",",
    "\"covariance\":\"Every unordered pair of genuine measurements, including explicit zero cross terms; no rows for unmeasured stocks; full symmetric positive-definite covariance in kg2\",",
    "\"basis\":\"kg-total-element/all-phases/full-physical-boundary\",",
    "\"values\":\"Quoted trimmed strings; masses are exact plain decimals or n/d with positive denominator; times signed integer nanoseconds; error parameters finite numbers\",",
    "\"output\":\"Original Claims, shared stock trajectory, every fitted flow, full posterior and adjustment covariance, exact observed contributions and period residuals\",",
    "\"diagnostics\":\"Zero degrees of freedom means no_redundancy with null chi_square, never a passed gross-error test; correlated adjustment diagnostics do not identify a unique faulty sensor\",",
    "\"rejected\":\"Physical or gross-error failure returns nonzero exit with original evidence, fitted diagnostics and rejected receipt digest, but no admitted Claim\",",
    "\"examples\":[\"tests/plant/trajectory_survey.toml\"]}"
);

type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;
fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    fail(
        RefusalCode::InputMissingOrUnreadable506,
        format!("worksheet line {line}: {}", detail.as_ref()),
    )
}
fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>> {
    let mut values = BTreeMap::new();
    for field in input {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(bad(field.line, format!("unsupported field {}", field.key)));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(bad(field.line, "values must be quoted strings"));
        };
        if !named(value) {
            return Err(bad(
                field.line,
                "values must be trimmed nonempty text of at most 4096 bytes",
            ));
        }
        values.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !values.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(values)
}
fn get<'a>(values: &Fields<'a>, key: &str) -> Result<(&'a str, usize)> {
    values
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}
fn integer(value: (&str, usize)) -> Result<i64> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected signed integer nanoseconds"))
}
fn boolean(value: (&str, usize)) -> Result<bool> {
    match value.0 {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad(value.1, "expected quoted true or false")),
    }
}
fn number(value: (&str, usize)) -> Result<f64> {
    let parsed = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite number"))?;
    if !parsed.is_finite() {
        return Err(bad(value.1, "expected a finite number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if parsed == 0.0 && mantissa.bytes().any(|b| matches!(b, b'1'..=b'9')) {
        return Err(numerical(
            "nonzero worksheet error parameter underflows binary64",
        ));
    }
    Ok(if parsed == 0.0 { 0.0 } else { parsed })
}
fn quantity(value: (&str, usize)) -> Result<ExactQty> {
    let invalid = || {
        bad(
            value.1,
            "mass requires a plain exact decimal or n/d with positive denominator",
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
    additivity: AdditivityClass,
    source: &SourceAnchor,
    locator: &str,
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
            format!("{}:{locator}", source.locator),
        )],
    )
}
fn input_claim(
    value: TrajectoryInput,
    source: &SourceAnchor,
    locator: &str,
) -> Claim<TrajectoryInput> {
    let tag = value.canonical_tag();
    let additivity = value.value.additivity();
    source_claim(value, &tag, INPUT_METRIC, additivity, source, locator)
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorksheetResult {
    submitted: String,
    source: SourceAnchor,
    outcome: TrajectoryOutcome,
}
impl WorksheetResult {
    pub fn outcome(&self) -> &TrajectoryOutcome {
        &self.outcome
    }
    pub fn refusal(&self) -> Option<&Refusal> {
        self.outcome.refusal()
    }
    pub fn to_json(&self) -> String {
        super::render::json(&self.submitted, &self.source, &self.outcome)
    }
    pub fn to_markdown(&self) -> String {
        super::render::markdown(&self.outcome)
    }
}

/// Preserve an unreadmitted submission for investigation without inventing a
/// typed source population, numerical candidate, Claim or receipt identity.
pub fn refusal_json(text: &str, source: &SourceAnchor, refusal: &Refusal) -> String {
    super::render::refusal_json(text, source, refusal)
}

/// Pure evaluation; no filesystem, clock, network or authority side effects.
pub fn evaluate(text: &str, source: &SourceAnchor) -> Result<WorksheetResult> {
    if text.len() as u64 > MAX_BYTES || !named(&source.artifact_path) || !named(&source.locator) {
        return Err(bad(1, "bounded worksheet and named source anchor required"));
    }
    if sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(fail(
            RefusalCode::DigestMismatch505,
            "submitted bytes differ from the source SHA-256",
        ));
    }
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    let max_rows = MAX_PERIODS + MAX_PERIODS + 1 + MAX_COORDINATES * (MAX_COORDINATES + 1) / 2;
    if document.rows.len() > max_rows {
        return Err(bad(
            1,
            "worksheet exceeds bounded period, stocktake and covariance population",
        ));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "element",
            "measurand",
            "basis",
            "start_ns",
            "end_ns",
            "complete",
            "error_model_basis",
            "unbiased_gaussian_errors",
            "global_chi_square_limit",
            "closure_tolerance_kg",
        ],
        &["normalized_adjustment_limit"],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "basis")?.0 != inventory::MASS_BASIS {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("basis must be {}", inventory::MASS_BASIS),
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
    let mut periods = Vec::new();
    let mut inputs = Vec::new();
    let mut missing = BTreeSet::new();
    let mut stock_count = 0;
    for row in &document.rows {
        match row.table.as_str() {
            "period" => {
                if periods.len() >= MAX_PERIODS {
                    return Err(bad(row.line, "at most 32 periods"));
                }
                let values = fields(
                    &row.fields,
                    &["start_ns", "end_ns", "inflow_kg", "outflow_kg"],
                    &[],
                    row.line,
                )?;
                let time = ValidTime {
                    start_ns: integer(get(&values, "start_ns")?)?,
                    end_ns: integer(get(&values, "end_ns")?)?,
                };
                periods.push(time);
                for (name, value) in [
                    (
                        "inflow_kg",
                        TrajectoryValue::Inflow(quantity(get(&values, "inflow_kg")?)?),
                    ),
                    (
                        "outflow_kg",
                        TrajectoryValue::Outflow(quantity(get(&values, "outflow_kg")?)?),
                    ),
                ] {
                    inputs.push(input_claim(
                        TrajectoryInput {
                            scope: scope.clone(),
                            physical_time: time,
                            value,
                        },
                        source,
                        &format!("line {}:{name}", row.line),
                    ));
                }
            }
            "stocktake" | "unmeasured_stocktake" => {
                stock_count += 1;
                if stock_count > MAX_PERIODS + 1 {
                    return Err(bad(row.line, "at most 33 shared stock boundaries"));
                }
                let required: &[&str] = if row.table == "stocktake" {
                    &["at_ns", "kg"]
                } else {
                    &["at_ns"]
                };
                let values = fields(&row.fields, required, &[], row.line)?;
                let time = integer(get(&values, "at_ns")?)?;
                if row.table == "stocktake" {
                    inputs.push(input_claim(
                        TrajectoryInput {
                            scope: scope.clone(),
                            physical_time: ValidTime::instant(time),
                            value: TrajectoryValue::Stocktake(quantity(get(&values, "kg")?)?),
                        },
                        source,
                        &format!("line {}:kg", row.line),
                    ));
                } else if !missing.insert(time) {
                    return Err(bad(row.line, "duplicate unmeasured stock boundary"));
                }
            }
            "covariance" => {}
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        }
    }
    periods.sort_by_key(|t| (t.start_ns, t.end_ns));
    if !(2..=MAX_PERIODS).contains(&periods.len()) {
        return Err(bad(1, "requires 2..32 period rows"));
    }
    if periods.iter().any(|t| t.start_ns >= t.end_ns)
        || periods.windows(2).any(|p| p[0].end_ns != p[1].start_ns)
        || periods[0].start_ns != scope.interval.start_ns
        || periods[periods.len() - 1].end_ns != scope.interval.end_ns
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "positive periods must tile the scope without gaps, overlaps or duplicate intervals",
        ));
    }
    let mut boundaries_ns = vec![periods[0].start_ns];
    boundaries_ns.extend(periods.iter().map(|p| p.end_ns));
    let declaration = input_claim(
        TrajectoryInput {
            scope: scope.clone(),
            physical_time: scope.interval,
            value: TrajectoryValue::Population {
                boundaries_ns,
                unmeasured_stocktakes: missing,
                complete: boolean(get(&top, "complete")?)?,
            },
        },
        source,
        "population and explicitly unmeasured boundary states",
    );
    let declaration_id = declaration.claim_id;
    inputs.push(declaration);
    let observations = observation_order(&inputs)?;
    let n = observations.len();
    let keys: BTreeMap<_, _> = observations
        .iter()
        .enumerate()
        .map(|(i, key)| (key.location.worksheet_key(), i))
        .collect();
    let mut covariance = vec![KilogramsSquared::new(0.0); n * n];
    let mut seen = BTreeSet::new();
    for row in document.rows_of("covariance") {
        let values = fields(&row.fields, &["left", "right", "kg2"], &[], row.line)?;
        let coordinate = |field| -> Result<usize> {
            let name = get(&values, field)?.0;
            keys.get(name).copied().ok_or_else(|| {
                bad(
                    row.line,
                    format!("{name} is not a genuine observed coordinate"),
                )
            })
        };
        let (i, j) = (coordinate("left")?, coordinate("right")?);
        if !seen.insert((i.min(j), i.max(j))) {
            return Err(bad(row.line, "duplicate covariance pair"));
        }
        let value = KilogramsSquared::new(number(get(&values, "kg2")?)?);
        covariance[i * n + j] = value;
        covariance[j * n + i] = value;
    }
    if seen.len() != n * (n + 1) / 2 {
        return Err(bad(
            1,
            "every covariance pair must be explicit, including zero cross terms",
        ));
    }
    let model = TrajectoryErrorModel {
        scope,
        declaration_id,
        observations,
        covariance,
        error_model_basis: get(&top, "error_model_basis")?.0.into(),
        unbiased_gaussian_errors: boolean(get(&top, "unbiased_gaussian_errors")?)?,
        global_chi_square_limit: number(get(&top, "global_chi_square_limit")?)?,
        normalized_adjustment_limit: top
            .get("normalized_adjustment_limit")
            .copied()
            .map(number)
            .transpose()?,
        closure_tolerance_kg: Kilograms::new(number(get(&top, "closure_tolerance_kg")?)?),
    };
    let tag = model.canonical_tag();
    let model = source_claim(
        model,
        &tag,
        ERROR_MODEL_METRIC,
        AdditivityClass::NonAdditive,
        source,
        "complete measurement covariance and Gaussian diagnostic policy",
    );
    Ok(WorksheetResult {
        submitted: text.into(),
        source: source.clone(),
        outcome: reconcile_trajectory(&inputs, &model)?,
    })
}
