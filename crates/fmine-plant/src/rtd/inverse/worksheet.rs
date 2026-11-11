//! DESIGN: source-bound worksheet for conditional known-kernel RTD deconvolution.
//! Determinism: DeterministicPerIsa

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use crate::flowsheet::reconciliation::unmeasured::GrossErrorDiagnostic;
use crate::flowsheet::{ElementKey, FlowsheetScope, MaterialAmounts};
use crate::reconciliation::{Kilograms, KilogramsSquared};
use crate::rtd::{
    CubicMetres, CubicMetresPerSecond, MODEL_METRIC, MaterialChannel, OPENING_METRIC, RtdModel,
    RtdOpening,
};

use super::{
    ERROR_MODEL_METRIC, INPUT_METRIC, InverseErrorModel, InverseInterval, InverseOutcome,
    MAX_INTERVALS, ObservationRole, deconvolve, observation_order,
};

type Result<T> = std::result::Result<T, Refusal>;
type Fields<'a> = BTreeMap<&'a str, (&'a str, usize)>;

pub const SCHEMA: &str = "fmine.plant-rtd-inverse.v1";
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const NO_CLAIM: &str = "Conditional deconvolution of one conservative elemental channel under a fixed declared tank-train response, integrated slurry volumes and explicit initial stocks. Only genuine outlet and optional feed measurements enter the supplied full covariance. Unmeasured feeds have no invented observation or regularization prior. Submitted values and Gaussian error assumptions remain Assumed, Internal and uncalibrated. No RTD parameter fitting, kernel or initial-stock uncertainty, reactive recovery, sensor-bias identification, empirical calibration, P754 compliance, ledger commit or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-rtd-inverse.v1\",\"command\":\"plant rtd-infer --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB; exact bytes SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"element\",\"measurand\",\"mass_basis\",\"start_ns\",\"end_ns\",\"opening_ns\",\"tanks\",\"volume_m3\",\"bypass_fraction\",\"dead_fraction\",\"min_flow_m3_per_s\",\"max_flow_m3_per_s\",\"common_conservative_transport\",\"piecewise_constant_feed_per_volume\",\"dry_tolerance_kg\",\"water_tolerance_kg\",\"element_tolerance_kg\",\"max_kernel_steps\",\"error_model_basis\",\"fixed_transport_and_opening\",\"unbiased_gaussian_errors\",\"global_chi_square_limit\",\"closure_tolerance_kg\"],",
    "\"optional_fields\":[\"normalized_adjustment_limit\"],",
    "\"mass_basis\":\"kg-total/all-phases/full-physical-boundary\",",
    "\"opening_rows\":[\"tank\",\"dry_kg\",\"water_kg\",\"element_kg\"],",
    "\"opening\":\"Exactly one explicit stock for each tank in 1..tanks and one tank=dead row; opening_ns equals start_ns\",",
    "\"interval_rows\":[\"start_ns\",\"end_ns\",\"slurry_volume_m3\",\"feed_kg\",\"outlet_kg\"],",
    "\"intervals\":\"1..32 positive contiguous intervals tile the full scope; every integrated outlet elemental mass is observed; feed_kg is an exact observation or the explicit string unmeasured\",",
    "\"covariance_rows\":[\"left\",\"right\",\"kg2\"],",
    "\"coordinate_keys\":\"feed/<start_ns>/<end_ns> or outlet/<start_ns>/<end_ns>; canonical signed integer nanoseconds; only genuine observations\",",
    "\"covariance\":\"Every unordered pair, including explicit zero cross terms; full symmetric positive-definite covariance in kg2; no rows for unmeasured feeds\",",
    "\"policy\":\"fixed_transport_and_opening and unbiased_gaussian_errors must be true; diagnostic limits are explicitly supplied; no hidden priors or physical clipping\",",
    "\"values\":\"Quoted trimmed strings; masses exact decimals or n/d with a positive denominator; finite model/error parameters\",",
    "\"output\":\"Original source Claims, outlet response matrix, fixed opening contribution, fitted feeds/outlets, full joint covariance, individual adjustments, inventory and conservation residuals\",",
    "\"diagnostics\":\"Zero degrees of freedom means no_redundancy with null chi_square; negative fitted mass and gross-error failure reject while retaining available diagnostics\",",
    "\"examples\":[\"tests/plant/rtd_inverse.toml\"]}"
);

fn fail(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant RTD inverse: {}", detail.into()))
}
fn bad(line: usize, detail: impl AsRef<str>) -> Refusal {
    fail(
        RefusalCode::InputMissingOrUnreadable506,
        format!("worksheet line {line}: {}", detail.as_ref()),
    )
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}
fn fields<'a>(
    input: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
    line: usize,
) -> Result<Fields<'a>> {
    let mut result = BTreeMap::new();
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
        result.insert(field.key.as_str(), (value.as_str(), field.line));
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(bad(line, format!("missing {key}")));
        }
    }
    Ok(result)
}
fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize)> {
    fields
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
fn count(value: (&str, usize)) -> Result<usize> {
    value
        .0
        .parse()
        .map_err(|_| bad(value.1, "expected an unsigned integer"))
}
fn boolean(value: (&str, usize)) -> Result<bool> {
    match value.0 {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(bad(value.1, "expected quoted true or false")),
    }
}
fn finite(value: (&str, usize)) -> Result<f64> {
    let result = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected a finite number"))?;
    if !result.is_finite() {
        return Err(bad(value.1, "expected a finite number"));
    }
    let mantissa = value.0.split(['e', 'E']).next().unwrap_or(value.0);
    if result == 0.0 && mantissa.bytes().any(|b| matches!(b, b'1'..=b'9')) {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "nonzero worksheet parameter underflows binary64",
        ));
    }
    Ok(if result == 0.0 { 0.0 } else { result })
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
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
        || (digits.contains('.') && fractional.is_empty())
    {
        return Err(invalid());
    }
    ExactQty::from_decimal_str(value.0).map_err(|_| invalid())
}
fn period(values: &Fields<'_>, boundary: &str) -> Result<FlowsheetScope> {
    Ok(FlowsheetScope {
        boundary: boundary.into(),
        interval: ValidTime {
            start_ns: integer(get(values, "start_ns")?)?,
            end_ns: integer(get(values, "end_ns")?)?,
        },
    })
}
fn source_claim<T>(
    value: T,
    tag: &str,
    metric: &str,
    additive: AdditivityClass,
    source: &SourceAnchor,
    locator: &str,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additive,
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

/// Original submission and complete admitted or rejected numerical result.
#[derive(Debug, Clone, PartialEq)]
pub struct WorksheetResult {
    submitted: String,
    source: SourceAnchor,
    outcome: InverseOutcome,
}

/// Pure parsing, inference and rendering; the adapter performs no file, clock or authority operation.
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
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    let max_observations = 2 * MAX_INTERVALS;
    if doc.rows.len() > 65 + MAX_INTERVALS + max_observations * (max_observations + 1) / 2 {
        return Err(bad(1, "too many opening, interval or covariance rows"));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "boundary",
            "element",
            "measurand",
            "mass_basis",
            "start_ns",
            "end_ns",
            "opening_ns",
            "tanks",
            "volume_m3",
            "bypass_fraction",
            "dead_fraction",
            "min_flow_m3_per_s",
            "max_flow_m3_per_s",
            "common_conservative_transport",
            "piecewise_constant_feed_per_volume",
            "dry_tolerance_kg",
            "water_tolerance_kg",
            "element_tolerance_kg",
            "max_kernel_steps",
            "error_model_basis",
            "fixed_transport_and_opening",
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
    if get(&top, "mass_basis")?.0 != "kg-total/all-phases/full-physical-boundary" {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "mass_basis must be kg-total/all-phases/full-physical-boundary",
        ));
    }
    let element = ElementKey {
        element: get(&top, "element")?.0.into(),
        measurand: get(&top, "measurand")?.0.into(),
    };
    let scope = period(&top, get(&top, "boundary")?.0)?;
    let tanks = count(get(&top, "tanks")?)?;
    if !(1..=64).contains(&tanks) {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "tanks must be in 1..64",
        ));
    }
    let model = RtdModel {
        scope: scope.clone(),
        elements: BTreeSet::from([element.clone()]),
        tanks,
        volume_m3: CubicMetres::new(finite(get(&top, "volume_m3")?)?),
        bypass_fraction: finite(get(&top, "bypass_fraction")?)?,
        dead_fraction: finite(get(&top, "dead_fraction")?)?,
        total_flow_domain_m3_per_s: [
            CubicMetresPerSecond::new(finite(get(&top, "min_flow_m3_per_s")?)?),
            CubicMetresPerSecond::new(finite(get(&top, "max_flow_m3_per_s")?)?),
        ],
        common_conservative_transport: boolean(get(&top, "common_conservative_transport")?)?,
        piecewise_constant_feed_per_volume: boolean(get(
            &top,
            "piecewise_constant_feed_per_volume",
        )?)?,
        closure_tolerances_kg: BTreeMap::from([
            (
                MaterialChannel::DrySolids,
                Kilograms::new(finite(get(&top, "dry_tolerance_kg")?)?),
            ),
            (
                MaterialChannel::Water,
                Kilograms::new(finite(get(&top, "water_tolerance_kg")?)?),
            ),
            (
                MaterialChannel::Element(element.clone()),
                Kilograms::new(finite(get(&top, "element_tolerance_kg")?)?),
            ),
        ]),
        max_kernel_steps: count(get(&top, "max_kernel_steps")?)?,
    };
    let tag = model.canonical_tag();
    let model = source_claim(
        model,
        &tag,
        MODEL_METRIC,
        AdditivityClass::NonAdditive,
        source,
        "fixed model and flow-domain declarations",
    );
    let mut opening_rows = BTreeMap::new();
    let mut dead = None;
    let mut intervals = Vec::new();
    for row in &doc.rows {
        match row.table.as_str() {
            "opening" => {
                let values = fields(
                    &row.fields,
                    &["tank", "dry_kg", "water_kg", "element_kg"],
                    &[],
                    row.line,
                )?;
                let stock = MaterialAmounts {
                    dry_solids_kg: quantity(get(&values, "dry_kg")?)?,
                    water_kg: quantity(get(&values, "water_kg")?)?,
                    elements_kg: BTreeMap::from([(
                        element.clone(),
                        quantity(get(&values, "element_kg")?)?,
                    )]),
                };
                let tank = get(&values, "tank")?;
                if tank.0 == "dead" {
                    if dead.replace(stock).is_some() {
                        return Err(bad(row.line, "duplicate dead opening stock"));
                    }
                } else {
                    let index = count(tank)?;
                    if !(1..=tanks).contains(&index) {
                        return Err(bad(row.line, "opening tank must be in 1..tanks or dead"));
                    }
                    if opening_rows.insert(index, stock).is_some() {
                        return Err(bad(row.line, "duplicate tank opening stock"));
                    }
                }
            }
            "interval" => {
                if intervals.len() >= MAX_INTERVALS {
                    return Err(bad(row.line, "at most 32 inverse intervals"));
                }
                let values = fields(
                    &row.fields,
                    &[
                        "start_ns",
                        "end_ns",
                        "slurry_volume_m3",
                        "feed_kg",
                        "outlet_kg",
                    ],
                    &[],
                    row.line,
                )?;
                let feed = get(&values, "feed_kg")?;
                let interval = InverseInterval {
                    scope: period(&values, &scope.boundary)?,
                    element: element.clone(),
                    slurry_volume_m3: CubicMetres::new(finite(get(&values, "slurry_volume_m3")?)?),
                    feed_kg: if feed.0 == "unmeasured" {
                        None
                    } else {
                        Some(quantity(feed)?)
                    },
                    outlet_kg: quantity(get(&values, "outlet_kg")?)?,
                };
                let tag = interval.canonical_tag();
                intervals.push(source_claim(interval, &tag, INPUT_METRIC, AdditivityClass::ExtensiveAdditive, source, &format!("interval at line {}; integrated elemental masses, slurry volume and support", row.line)));
            }
            "covariance" => {}
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        }
    }
    if opening_rows.len() != tanks {
        return Err(bad(
            1,
            "every tank opening stock is required, including explicit zero totals",
        ));
    }
    let opening = RtdOpening {
        boundary: scope.boundary.clone(),
        at_ns: integer(get(&top, "opening_ns")?)?,
        tanks: opening_rows.into_values().collect(),
        dead: dead.ok_or_else(|| {
            bad(
                1,
                "an explicit dead opening stock is required, including zeros",
            )
        })?,
    };
    let tag = opening.canonical_tag();
    let opening = source_claim(
        opening,
        &tag,
        OPENING_METRIC,
        AdditivityClass::ExtensiveAdditive,
        source,
        "all opening stocks and opening_ns",
    );
    intervals.sort_by_key(|c| {
        (
            c.value.scope.interval.start_ns,
            c.value.scope.interval.end_ns,
        )
    });
    let observations = observation_order(&intervals)?;
    let mut keys = BTreeMap::new();
    for (i, observation) in observations.iter().enumerate() {
        let interval = intervals
            .iter()
            .find(|c| c.claim_id == observation.interval_source)
            .ok_or_else(|| bad(1, "measurement has no interval source"))?;
        let key = coordinate_key(observation.role, interval.value.scope.interval);
        if keys.insert(key, i).is_some() {
            return Err(bad(1, "duplicate interval observation"));
        }
    }
    let n = observations.len();
    let mut covariance = vec![KilogramsSquared::new(0.0); n * n];
    let mut seen = BTreeSet::new();
    for row in doc.rows_of("covariance") {
        let values = fields(&row.fields, &["left", "right", "kg2"], &[], row.line)?;
        let coordinate = |field| -> Result<usize> {
            let key = get(&values, field)?.0;
            keys.get(key).copied().ok_or_else(|| {
                bad(
                    row.line,
                    format!("{key} is not a genuine observed coordinate"),
                )
            })
        };
        let (i, j) = (coordinate("left")?, coordinate("right")?);
        if !seen.insert((i.min(j), i.max(j))) {
            return Err(bad(row.line, "duplicate covariance pair"));
        }
        let value = KilogramsSquared::new(finite(get(&values, "kg2")?)?);
        covariance[i * n + j] = value;
        covariance[j * n + i] = value;
    }
    if seen.len() != n * (n + 1) / 2 {
        return Err(bad(
            1,
            "every covariance pair must be explicit, including zero cross terms",
        ));
    }
    let errors = InverseErrorModel {
        scope,
        element,
        model_source: model.claim_id,
        opening_source: opening.claim_id,
        observations,
        covariance,
        error_model_basis: get(&top, "error_model_basis")?.0.into(),
        fixed_transport_and_opening: boolean(get(&top, "fixed_transport_and_opening")?)?,
        unbiased_gaussian_errors: boolean(get(&top, "unbiased_gaussian_errors")?)?,
        global_chi_square_limit: finite(get(&top, "global_chi_square_limit")?)?,
        normalized_adjustment_limit: top
            .get("normalized_adjustment_limit")
            .copied()
            .map(finite)
            .transpose()?,
        closure_tolerance_kg: Kilograms::new(finite(get(&top, "closure_tolerance_kg")?)?),
    };
    let tag = errors.canonical_tag();
    let errors = source_claim(
        errors,
        &tag,
        ERROR_MODEL_METRIC,
        AdditivityClass::NonAdditive,
        source,
        "full measurement covariance and fixed-model Gaussian diagnostic policy",
    );
    Ok(WorksheetResult {
        submitted: text.into(),
        source: source.clone(),
        outcome: deconvolve(&model, &opening, &intervals, &errors, || false)?,
    })
}

fn coordinate_key(role: ObservationRole, time: ValidTime) -> String {
    let role = match role {
        ObservationRole::Feed => "feed",
        ObservationRole::Outlet => "outlet",
    };
    format!("{role}/{}/{}", time.start_ns, time.end_ns)
}
fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace(['\n', '\r'], " ")
}
fn number(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn numbers(values: impl Iterator<Item = f64>) -> String {
    format!("[{}]", values.map(number).collect::<Vec<_>>().join(","))
}
fn exact(value: ExactQty) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
fn source_json(source: &SourceAnchor) -> String {
    format!(
        "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        quoted(&source.locator)
    )
}
fn claim_json<T>(claim: &Claim<T>) -> String {
    let sources = claim
        .anchors
        .iter()
        .map(source_json)
        .collect::<Vec<_>>()
        .join(",");
    let parents = claim
        .parents
        .iter()
        .map(|id| quoted(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let mut body = String::new();
    for b in claim.canonical_body() {
        let _ = write!(body, "{b:02x}");
    }
    format!(
        concat!(
            "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"representation\":{},",
            "\"support\":{},\"additivity\":{},\"evidence\":{},\"epistemic\":{},\"state\":{},\"confidentiality\":{},",
            "\"calibration\":\"uncalibrated\",\"stale\":{},\"calibration_failed\":{},\"parents\":[{parents}],\"sources\":[{sources}]}}"
        ),
        quoted(&claim.claim_id.to_string()),
        quoted(&claim.metric_key),
        quoted(claim.value_tag()),
        quoted(&body),
        quoted(&format!("{:?}", claim.representation)),
        quoted(&format!("{:?}", claim.support)),
        quoted(&format!("{:?}", claim.additivity)),
        quoted(&format!("{:?}", claim.evidence)),
        quoted(&format!("{:?}", claim.epistemic)),
        quoted(&format!("{:?}", claim.state)),
        quoted(&claim.confidentiality.to_string()),
        claim.stale,
        claim.calibration_failed,
        parents = parents,
        sources = sources,
    )
}
fn gross(diagnostic: &GrossErrorDiagnostic) -> String {
    match diagnostic {
        GrossErrorDiagnostic::NoRedundancy => {
            "{\"status\":\"no_redundancy\",\"chi_square\":null,\"degrees_of_freedom\":0}".into()
        }
        GrossErrorDiagnostic::Available {
            chi_square,
            degrees_of_freedom,
        } => format!(
            "{{\"status\":\"available\",\"chi_square\":{},\"degrees_of_freedom\":{degrees_of_freedom}}}",
            number(*chi_square)
        ),
    }
}
fn refusal_value(refusal: &Refusal) -> String {
    format!(
        "{{\"code\":{},\"what\":{},\"fix\":{}}}",
        quoted(refusal.code),
        quoted(&refusal.what),
        quoted(refusal.fix)
    )
}

/// Unreadmitted source bytes remain inspectable without an invented typed population or receipt.
pub fn refusal_json(text: &str, source: &SourceAnchor, refusal: &Refusal) -> String {
    format!(
        concat!(
            "{{\"schema\":\"fmine.plant-rtd-inverse-result.v1\",\"source\":{},\"submitted_worksheet\":{},",
            "\"model\":null,\"opening\":null,\"source_inputs\":null,\"error_model\":null,\"result\":null,",
            "\"admitted_claim\":null,\"receipt_digest\":null,\"refusal\":{},\"no_claim\":{}}}"
        ),
        source_json(source),
        quoted(text),
        refusal_value(refusal),
        quoted(NO_CLAIM)
    )
}

impl WorksheetResult {
    pub fn outcome(&self) -> &InverseOutcome {
        &self.outcome
    }
    pub fn refusal(&self) -> Option<&Refusal> {
        self.outcome.refusal()
    }

    pub fn to_json(&self) -> String {
        let outcome = &self.outcome;
        let d = outcome.diagnostics();
        let e = &outcome.error_model().value;
        let model = &outcome.model().value;
        let inputs = outcome.intervals().iter().map(|c| format!(
            "{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"slurry_volume_m3\":{},\"feed_kg_exact\":{},\"outlet_kg_exact\":{},\"claim\":{}}}",
            c.value.scope.interval.start_ns, c.value.scope.interval.end_ns, number(c.value.slurry_volume_m3.value()),
            c.value.feed_kg.map_or_else(|| "null".into(), exact), exact(c.value.outlet_kg), claim_json(c),
        )).collect::<Vec<_>>().join(",");
        let order = d
            .observations
            .iter()
            .filter_map(|o| {
                outcome
                    .intervals()
                    .iter()
                    .find(|c| c.claim_id == o.interval_source)
                    .map(|c| {
                        format!(
                            "{{\"key\":{},\"source_claim_id\":{},\"unmeasured\":false}}",
                            quoted(&coordinate_key(o.role, c.value.scope.interval)),
                            quoted(&o.interval_source.to_string())
                        )
                    })
            })
            .chain(d.unmeasured_feeds.iter().filter_map(|id| {
                outcome
                    .intervals()
                    .iter()
                    .find(|c| c.claim_id == *id)
                    .map(|c| {
                        format!(
                            "{{\"key\":{},\"source_claim_id\":{},\"unmeasured\":true}}",
                            quoted(&coordinate_key(
                                ObservationRole::Feed,
                                c.value.scope.interval
                            )),
                            quoted(&id.to_string())
                        )
                    })
            }))
            .collect::<Vec<_>>()
            .join(",");
        let periods = outcome.intervals().iter().enumerate().map(|(i, c)| format!(
            concat!("{{\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"feed_coordinate_index\":{},\"outlet_coordinate_index\":{},",
            "\"fitted_feed_kg\":{},\"fitted_outlet_kg\":{},\"predicted_outlet_kg\":{},\"opening_outlet_kg\":{},",
            "\"closing_inventory_kg\":{},\"transport_residual_kg\":{},\"conservation_residual_kg\":{}}}"),
            c.value.scope.interval.start_ns, c.value.scope.interval.end_ns, d.feed_coordinate_indices[i], d.outlet_coordinate_indices[i],
            number(d.fitted_feed_kg[i].value()), number(d.fitted_outlet_kg[i].value()), number(d.predicted_outlet_kg[i].value()),
            number(d.opening_outlet_kg[i].value()), number(d.closing_inventory_kg[i].value()),
            number(d.transport_residual_kg[i].value()), number(d.conservation_residual_kg[i].value()),
        )).collect::<Vec<_>>().join(",");
        let normalized = d
            .normalized_adjustments
            .iter()
            .map(|v| v.map_or_else(|| "null".into(), number))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema\":\"fmine.plant-rtd-inverse-result.v1\",\"source\":{},\"submitted_worksheet\":{},",
                "\"scope\":{{\"boundary\":{},\"element\":{},\"measurand\":{},\"mass_basis\":\"kg-total-element/all-phases/full-physical-boundary\",\"start_ns\":\"{}\",\"end_ns\":\"{}\"}},",
                "\"model\":{{\"claim\":{},\"tanks\":{},\"volume_m3\":{},\"bypass_fraction\":{},\"dead_fraction\":{},\"max_kernel_steps\":{}}},",
                "\"opening\":{},\"source_inputs\":[{inputs}],\"coordinate_order\":[{order}],\"measured_coordinate_count\":{},",
                "\"error_model\":{{\"claim\":{},\"basis\":{},\"fixed_transport_and_opening\":true,\"unbiased_gaussian_errors\":true,",
                "\"covariance_kg2\":{},\"global_chi_square_limit\":{},\"normalized_adjustment_limit\":{},\"closure_tolerance_kg\":{}}},",
                "\"result\":{{\"intervals\":[{periods}],\"observed_kg\":{},\"estimates_kg\":{},\"measured_adjustments_kg\":{},",
                "\"posterior_covariance_kg2\":{},\"measured_adjustment_covariance_kg2\":{},\"normalized_adjustments\":[{normalized}],",
                "\"response_matrix\":{},\"response_matrix_rows\":{},\"response_matrix_columns\":{},\"raw_observed_residual_kg\":{},",
                "\"whole_window_residual_kg\":{},\"gross_error\":{},\"kernel_steps\":{}}},",
                "\"admitted_claim\":{},\"refusal\":{},\"receipt_digest\":{},\"calibration\":\"uncalibrated\",\"no_claim\":{}}}"
            ),
            source_json(&self.source),
            quoted(&self.submitted),
            quoted(&d.scope.boundary),
            quoted(&d.element.element),
            quoted(&d.element.measurand),
            d.scope.interval.start_ns,
            d.scope.interval.end_ns,
            claim_json(outcome.model()),
            model.tanks,
            number(model.volume_m3.value()),
            number(model.bypass_fraction),
            number(model.dead_fraction),
            model.max_kernel_steps,
            claim_json(outcome.opening()),
            d.observations.len(),
            claim_json(outcome.error_model()),
            quoted(&e.error_model_basis),
            numbers(e.covariance.iter().map(|v| v.value())),
            number(e.global_chi_square_limit),
            e.normalized_adjustment_limit
                .map_or_else(|| "null".into(), number),
            number(e.closure_tolerance_kg.value()),
            numbers(d.observed_kg.iter().map(|v| v.value())),
            numbers(d.estimates_kg.iter().map(|v| v.value())),
            numbers(d.measured_adjustments_kg.iter().map(|v| v.value())),
            numbers(d.posterior_covariance.iter().map(|v| v.value())),
            numbers(d.measured_adjustment_covariance.iter().map(|v| v.value())),
            numbers(d.response_matrix.iter().copied()),
            outcome.intervals().len(),
            outcome.intervals().len(),
            numbers(d.raw_observed_residual_kg.iter().map(|v| v.value())),
            number(d.whole_window_residual_kg.value()),
            gross(&d.gross_error),
            d.kernel_steps,
            outcome
                .admitted_claim()
                .map_or_else(|_| "null".into(), claim_json),
            outcome
                .refusal()
                .map_or_else(|| "null".into(), refusal_value),
            quoted(outcome.digest()),
            quoted(NO_CLAIM),
            inputs = inputs,
            order = order,
            periods = periods,
            normalized = normalized,
        )
    }

    pub fn to_markdown(&self) -> String {
        let outcome = &self.outcome;
        let d = outcome.diagnostics();
        let n = d.estimates_kg.len();
        let mut out = format!(
            "# Conditional residence-time deconvolution\n\n{NO_CLAIM}\n\nBoundary: {}. Measurand: {} ({}).\n\n",
            escaped(&d.scope.boundary),
            escaped(&d.element.measurand),
            escaped(&d.element.element),
        );
        if let Some(refusal) = outcome.refusal() {
            let _ = writeln!(
                out,
                "**Rejected: {} — {}.** No admitted Claim; the fitted candidate remains available for investigation.\n",
                refusal.code,
                escaped(&refusal.what)
            );
        }
        out.push_str("## Reconstructed feed and observed outlet\n\nStandard deviations are conditional on the submitted fixed model and covariance; they do not establish empirical coverage.\n\n| Interval (ns) | Feed observation (kg) | Fitted feed (kg) | Feed standard deviation (kg) | Observed outlet (kg) | Fitted outlet (kg) | Model outlet (kg) | Closing inventory (kg) |\n|---|---:|---:|---:|---:|---:|---:|---:|\n");
        for (i, c) in outcome.intervals().iter().enumerate() {
            let f = d.feed_coordinate_indices[i];
            let observed_feed = c.value.feed_kg.map_or_else(
                || "Unmeasured, inferred".into(),
                |v| format!("{}/{}", v.num(), v.den()),
            );
            let _ = writeln!(
                out,
                "| {}..{} | {} | {} | {} | {}/{} | {} | {} | {} |",
                c.value.scope.interval.start_ns,
                c.value.scope.interval.end_ns,
                observed_feed,
                number(d.fitted_feed_kg[i].value()),
                number(d.posterior_covariance[f * n + f].value().sqrt()),
                c.value.outlet_kg.num(),
                c.value.outlet_kg.den(),
                number(d.fitted_outlet_kg[i].value()),
                number(d.predicted_outlet_kg[i].value()),
                number(d.closing_inventory_kg[i].value())
            );
        }
        out.push_str("\n## Measurement adjustments\n\n| Coordinate | Observed kg | Fitted kg | Adjustment kg | Normalized adjustment |\n|---|---:|---:|---:|---:|\n");
        for (i, o) in d.observations.iter().enumerate() {
            if let Some(c) = outcome
                .intervals()
                .iter()
                .find(|c| c.claim_id == o.interval_source)
            {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} |",
                    coordinate_key(o.role, c.value.scope.interval),
                    number(d.observed_kg[i].value()),
                    number(d.estimates_kg[i].value()),
                    number(d.measured_adjustments_kg[i].value()),
                    d.normalized_adjustments[i].map_or_else(|| "Undefined".into(), number)
                );
            }
        }
        out.push_str("\n## Conservation and gross-error diagnostics\n\n| Interval (ns) | Outlet model residual (kg) | Conservation residual (kg) |\n|---|---:|---:|\n");
        for (i, c) in outcome.intervals().iter().enumerate() {
            let _ = writeln!(
                out,
                "| {}..{} | {} | {} |",
                c.value.scope.interval.start_ns,
                c.value.scope.interval.end_ns,
                number(d.transport_residual_kg[i].value()),
                number(d.conservation_residual_kg[i].value())
            );
        }
        let _ = writeln!(
            out,
            "\nWhole-window conservation residual: {} kg.\n",
            number(d.whole_window_residual_kg.value())
        );
        match d.gross_error {
            GrossErrorDiagnostic::NoRedundancy => out.push_str("No redundant measurement remains after fitting the unknown feeds. A gross-error test is unavailable; this is not a passed test.\n"),
            GrossErrorDiagnostic::Available { chi_square, degrees_of_freedom } => {
                let _ = writeln!(out, "Conditional chi-square: {}; degrees of freedom: {}; declared limit: {}. Correlated adjustment diagnostics do not identify a uniquely faulty sensor.\n", number(chi_square), degrees_of_freedom, number(outcome.error_model().value.global_chi_square_limit));
            }
        }
        out.push_str("\nThe response matrix and full joint posterior covariance are retained in JSON.\n\n## Source and receipt\n\n");
        let _ = writeln!(
            out,
            "Source: {}. SHA-256: `{}`.\n\nReceipt: `{}`.\n",
            escaped(&self.source.artifact_path),
            Digest32(self.source.artifact_sha256).to_hex(),
            outcome.digest()
        );
        out
    }
}
