//! DESIGN (BD-151/155/177): one-boundary raw-measurement reconciliation.
//! Latent dry matter D, water W and elemental mass E conserve linearly. Wet
//! weight, moisture and assay observe D+W, W/(D+W) and E/D; grades are derived,
//! never adjusted conservation variables (plan section 20.2). Endpoint stocks
//! are explicitly fixed conditional inputs. This is a bounded local calculation.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;

use fmine_claim::{AdditivityClass, Claim};
use fmine_core::{
    Digest32,
    anchor::SourceAnchor,
    codec::Encoder,
    hash::{blake3_hash, sha256},
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::nonlinear::{NonlinearProblem, reconcile_nonlinear};
use fmine_registry::toml_subset::parse_subset;
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use crate::inventory::{
    BalanceScope, put_source_envelope,
    worksheet::{bad, declared, exact, fields, get, integer, quantity, quoted},
};
use crate::reconciliation::{Kilograms, KilogramsSquared, Metadata, represented_rational};

pub const MAX_BYTES: u64 = 1024 * 1024;
pub const SCHEMA: &str = "fmine.plant-raw-balance.v1";
pub const NO_CLAIM: &str = "Conditional local nonlinear reconciliation of extensive dry matter, H2O and elemental kilograms against original wet-weight, moisture and dry-assay observations. Full declared raw-error covariance and exact conditional endpoint stocks are retained. Grades and moisture are derived from fitted extensive quantities. Local Gauss-Newton tangent covariance is a first-order approximation, not a nonlinear posterior, calibrated interval or exact chi-square law. No global optimum, sensor-bias attribution, missing-observation inference, P754 qualification, source verification, ledger commit or operating authority is supplied. Boundary optima requiring active inequality multipliers refuse rather than clamp.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-raw-balance.v1\",\"command\":\"plant reconcile-raw --input <worksheet.toml>\",",
    "\"input\":\"Strict quoted-value TOML, at most 1 MiB; original bytes and SHA-256 retained\",",
    "\"scope\":\"One complete conservative mechanical boundary, one non-H/O element, positive interval, 2 to 16 whole-interval streams\",",
    "\"fields\":[\"schema\",\"boundary\",\"element\",\"measurand\",\"dry_basis\",\"start_ns\",\"end_ns\",\"mass_unit\",\"fraction_unit\",\"moisture_basis\",\"assay_basis\",\"complete\",\"whole_interval_streams\",\"conservative_mechanical_boundary\",\"moisture_is_h2o\",\"fixed_endpoint_stocks\",\"unbiased_gaussian_raw_errors\",\"local_covariance_accepted\",\"error_model_basis\",\"opening_dry_kg\",\"opening_water_kg\",\"opening_element_kg\",\"closing_dry_kg\",\"closing_water_kg\",\"closing_element_kg\",\"balance_tolerance_kg\",\"max_standardized_misfit\",\"max_iterations\"],",
    "\"streams\":\"[[stream]] id,direction=in|out,wet_kg,water_fraction,dry_element_fraction,sigma_wet_kg,sigma_water_fraction,sigma_dry_element_fraction; strictly positive uncertainties\",",
    "\"correlations\":\"[[correlation]] left,right,value; ALL unordered pairs including diagonals and explicit zeros, keys <stream>/wet_mass|moisture|assay\",",
    "\"units\":\"mass_unit=kg, fraction_unit=mass_fraction, moisture_basis=water_over_wet_mass, assay_basis=total_element/all_non_water_dry_matter\",",
    "\"admission\":\"Every declaration flag is explicitly true; positive wet mass, water fraction in [0,1), dry elemental fraction in [0,1], exact conditional physical endpoint stocks\",",
    "\"solver\":\"Full covariance, analytic nonlinear raw likelihood, linear extensive conservation, physical-domain merit line search; 1-128 iterations, information-scaled KKT and step tolerance 1e-9\",",
    "\"output\":\"Original bytes/Claims, nominal exact imbalance, fitted D/W/E, derived observations, complete local kg2 covariance, raw standardized residuals, objective, convergence audits and refusal diagnostics\",",
    "\"example\":\"tests/plant/raw_balance.toml\"}"
);

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
const CHANNELS: [&str; 3] = ["wet_mass", "moisture", "assay"];

struct Stream {
    id: String,
    direction: String,
    raw: [ExactQty; 3],
    sigma: [f64; 3],
}

struct Parsed {
    scope: BalanceScope,
    dry_basis: String,
    streams: Vec<Stream>,
    opening: [ExactQty; 3],
    closing: [ExactQty; 3],
    correlation: Vec<f64>,
    tolerance: f64,
    limit: f64,
    iterations: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciledStream {
    pub id: String,
    pub direction: String,
    pub dry_kg: Kilograms,
    pub water_kg: Kilograms,
    pub elemental_kg: Kilograms,
    pub predicted_wet_kg: Kilograms,
    pub derived_water_fraction: f64,
    pub derived_dry_element_fraction: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawBalanceSummary {
    pub scope: BalanceScope,
    pub dry_basis: String,
    /// Conditional endpoint stocks, each ordered [dry matter, H2O, element].
    pub fixed_opening_kg: [ExactQty; 3],
    pub fixed_closing_kg: [ExactQty; 3],
    pub streams: Vec<ReconciledStream>,
    pub raw_residuals_exact_kg: [ExactQty; 3],
    pub residuals_kg: [Kilograms; 3],
    /// Canonical stream order, each [dry matter, H2O, element]; all entries kg².
    pub local_extensive_covariance: Vec<KilogramsSquared>,
    pub standardized_measurement_residuals: Vec<f64>,
    pub standardized_misfit: f64,
    pub relative_stationarity: f64,
    pub standardized_step: f64,
    pub iterations: usize,
    pub backtracks: usize,
}

pub struct WorksheetResult {
    source: Claim<String>,
    parsed: Parsed,
    diagnostics: Option<RawBalanceSummary>,
    claim: Option<Claim<RawBalanceSummary>>,
    refusal: Option<Refusal>,
    digest: String,
}

impl WorksheetResult {
    pub fn refusal(&self) -> Option<&Refusal> {
        self.refusal.as_ref()
    }
    pub fn admitted_claim(&self) -> Option<&Claim<RawBalanceSummary>> {
        self.claim.as_ref()
    }
    pub fn diagnostics(&self) -> Option<&RawBalanceSummary> {
        self.diagnostics.as_ref()
    }
    pub fn source_claim(&self) -> &Claim<String> {
        &self.source
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn to_json(&self) -> String {
        let inputs = self.parsed.streams.iter().map(|s| {
            format!(
                "{{\"id\":{},\"direction\":{},\"observed\":[{}],\"standard_uncertainties\":{}}}",
                quoted(&s.id), quoted(&s.direction), exacts(&s.raw), numbers(&s.sigma)
            )
        }).collect::<Vec<_>>().join(",");
        let claim = self.claim.as_ref().map_or_else(
            || "null".into(),
            |c| {
                format!(
                    "{{\"id\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"confidentiality\":\"Internal\",\"calibration\":\"uncalibrated\",\"parents\":[{}]}}",
                    quoted(&c.claim_id.to_string()), quoted(&self.source.claim_id.to_string())
                )
            },
        );
        let refusal = self.refusal.as_ref().map_or_else(
            || "null".into(),
            |r| format!("{{\"code\":{},\"what\":{}}}", quoted(r.code), quoted(&r.what)),
        );
        let anchors = self.source.anchors.iter().map(|a| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                quoted(&a.artifact_path), quoted(&Digest32(a.artifact_sha256).to_hex()),
                quoted(&a.locator)
            )
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema\":\"fmine.plant-raw-balance-result.v1\",\"receipt_digest\":{},\"source_claim_id\":{},\"source_bytes\":{},\"sources\":[{}],\"inputs\":[{}],\"observation_order\":\"canonical stream; wet_mass,moisture,assay\",\"correlation\":{},\"diagnostics\":{},\"admitted_claim\":{},\"refusal\":{},\"no_claim\":{}}}",
            quoted(&self.digest), quoted(&self.source.claim_id.to_string()),
            quoted(&self.source.value), anchors, inputs, numbers(&self.parsed.correlation),
            self.diagnostics.as_ref().map_or_else(|| "null".into(), summary_json),
            claim, refusal, quoted(NO_CLAIM)
        )
    }
    pub fn to_markdown(&self) -> String {
        let mut out = String::from("# Raw-measurement plant reconciliation\n\n");
        if let Some(r) = &self.refusal {
            out.push_str(&format!("**Refused: {}** — {}\n\n", r.code, r.what));
        }
        if let Some(s) = &self.diagnostics {
            out.push_str("| Stream | Direction | Dry kg | H2O kg | Element kg | Derived dry fraction |\n|---|---|---:|---:|---:|---:|\n");
            for row in &s.streams {
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    row.id, row.direction, row.dry_kg.value(), row.water_kg.value(),
                    row.elemental_kg.value(), row.derived_dry_element_fraction
                ));
            }
            out.push_str(&format!(
                "\nStandardized misfit: {}. Iterations: {}. KKT residual: {}.\n\n",
                s.standardized_misfit, s.iterations, s.relative_stationarity
            ));
        }
        out.push_str(&format!("Receipt: {}.\n\n{}\n", self.digest, NO_CLAIM));
        out
    }
}

fn fail(what: &str) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("raw plant balance: {what}"),
    )
}
fn arithmetic(_: fmine_units::UnitsError) -> Refusal {
    fail("exact quantity arithmetic exceeded its range")
}
fn number(value: (&str, usize)) -> Result<f64> {
    let result = value
        .0
        .parse::<f64>()
        .map_err(|_| bad(value.1, "expected finite binary64 parameter"))?;
    let nonzero = value
        .0
        .split(['e', 'E'])
        .next()
        .unwrap_or("")
        .bytes()
        .any(|b| matches!(b, b'1'..=b'9'));
    if !result.is_finite() || (result == 0.0 && nonzero) {
        return Err(bad(value.1, "nonfinite or underflowing parameter"));
    }
    Ok(result)
}
fn nominal(q: ExactQty, budget: f64) -> Result<f64> {
    let value = q.to_f64();
    if !value.is_finite() || (q != ZERO && value == 0.0) {
        return Err(fail("exact input is outside numerical range"));
    }
    let loss = q
        .checked_sub(represented_rational(value)?)
        .map_err(arithmetic)?
        .to_f64()
        .abs();
    if loss > budget {
        return Err(fail(
            "input conversion exceeds its declared numerical precision budget",
        ));
    }
    Ok(value)
}
fn amounts(raw: [ExactQty; 3]) -> Result<[ExactQty; 3]> {
    let dry = raw[0]
        .checked_mul(ONE.checked_sub(raw[1]).map_err(arithmetic)?)
        .map_err(arithmetic)?;
    Ok([
        dry,
        raw[0].checked_mul(raw[1]).map_err(arithmetic)?,
        dry.checked_mul(raw[2]).map_err(arithmetic)?,
    ])
}

fn parse(text: &str) -> Result<Parsed> {
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if doc.rows.len() > 1200 {
        return Err(bad(1, "at most 1200 rows"));
    }
    let required = [
        "schema", "boundary", "element", "measurand", "dry_basis", "start_ns", "end_ns",
        "mass_unit", "fraction_unit", "moisture_basis", "assay_basis", "complete",
        "whole_interval_streams", "conservative_mechanical_boundary", "moisture_is_h2o",
        "fixed_endpoint_stocks", "unbiased_gaussian_raw_errors", "local_covariance_accepted",
        "error_model_basis", "opening_dry_kg", "opening_water_kg", "opening_element_kg",
        "closing_dry_kg", "closing_water_kg", "closing_element_kg", "balance_tolerance_kg",
        "max_standardized_misfit", "max_iterations",
    ];
    let top = fields(&doc.top, &required, &[], 1)?;
    for (key, expected) in [
        ("schema", SCHEMA),
        ("mass_unit", "kg"),
        ("fraction_unit", "mass_fraction"),
        ("moisture_basis", "water_over_wet_mass"),
        ("assay_basis", "total_element/all_non_water_dry_matter"),
    ] {
        if get(&top, key)?.0 != expected {
            return Err(bad(1, format!("{key} must be {expected}")));
        }
    }
    for key in [
        "complete", "whole_interval_streams", "conservative_mechanical_boundary",
        "moisture_is_h2o", "fixed_endpoint_stocks", "unbiased_gaussian_raw_errors",
        "local_covariance_accepted",
    ] {
        if get(&top, key)?.0 != "true" {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("explicit {key}=true is required"),
            ));
        }
    }
    let element = get(&top, "element")?.0;
    let symbol = element.as_bytes();
    if !(1..=2).contains(&symbol.len())
        || !symbol[0].is_ascii_uppercase()
        || (symbol.len() == 2 && !symbol[1].is_ascii_lowercase())
        || matches!(element, "H" | "O")
    {
        return Err(bad(
            1,
            "declare a case-sensitive non-H/O element; water stoichiometry is not inferred",
        ));
    }
    let interval = ValidTime {
        start_ns: integer(get(&top, "start_ns")?)?,
        end_ns: integer(get(&top, "end_ns")?)?,
    };
    if interval.start_ns >= interval.end_ns {
        return Err(bad(1, "positive-duration interval required"));
    }
    let scope = BalanceScope {
        boundary: get(&top, "boundary")?.0.into(),
        element: element.into(),
        measurand: get(&top, "measurand")?.0.into(),
        interval,
    };
    let dry_basis = get(&top, "dry_basis")?.0.into();
    let stock = |prefix: &str| -> Result<[ExactQty; 3]> {
        let q = [
            quantity(get(&top, &format!("{prefix}_dry_kg"))?)?,
            quantity(get(&top, &format!("{prefix}_water_kg"))?)?,
            quantity(get(&top, &format!("{prefix}_element_kg"))?)?,
        ];
        if q.iter().any(|v| v.signum() < 0)
            || q[2].checked_cmp(q[0]).map_err(arithmetic)?.is_gt()
        {
            return Err(bad(
                1,
                "endpoint stocks must be nonnegative extensive masses with element <= dry matter",
            ));
        }
        Ok(q)
    };
    let opening = stock("opening")?;
    let closing = stock("closing")?;
    let tolerance = number(get(&top, "balance_tolerance_kg")?)?;
    let limit = number(get(&top, "max_standardized_misfit")?)?;
    let iterations = get(&top, "max_iterations")?
        .0
        .parse::<usize>()
        .map_err(|_| bad(1, "invalid max_iterations"))?;
    if tolerance <= 0.0 || limit <= 0.0 || !(1..=128).contains(&iterations) {
        return Err(bad(
            1,
            "positive tolerance/misfit limit and 1-128 iterations required",
        ));
    }
    let mut by_id = BTreeMap::new();
    for row in &doc.rows {
        if row.table == "correlation" {
            continue;
        }
        if row.table != "stream" {
            return Err(bad(row.line, "only stream and correlation tables are supported"));
        }
        let f = fields(
            &row.fields,
            &[
                "id", "direction", "wet_kg", "water_fraction", "dry_element_fraction",
                "sigma_wet_kg", "sigma_water_fraction", "sigma_dry_element_fraction",
            ],
            &[],
            row.line,
        )?;
        let id = get(&f, "id")?.0;
        if id.len() > 128 || id.contains('/') || id.contains('|') {
            return Err(bad(row.line, "stream id must be <=128 bytes without / or |"));
        }
        let direction = get(&f, "direction")?.0;
        let raw = [
            quantity(get(&f, "wet_kg")?)?,
            quantity(get(&f, "water_fraction")?)?,
            quantity(get(&f, "dry_element_fraction")?)?,
        ];
        let sigma = [
            number(get(&f, "sigma_wet_kg")?)?,
            number(get(&f, "sigma_water_fraction")?)?,
            number(get(&f, "sigma_dry_element_fraction")?)?,
        ];
        if !matches!(direction, "in" | "out")
            || raw[0].signum() <= 0
            || raw[1].signum() < 0
            || raw[1].num() >= raw[1].den()
            || raw[2].signum() < 0
            || raw[2].num() > raw[2].den()
            || sigma.iter().any(|v| *v <= 0.0)
        {
            return Err(bad(
                row.line,
                "in/out direction, positive wet kg/uncertainties, water fraction [0,1) and dry assay [0,1] required",
            ));
        }
        let stream = Stream {
            id: id.into(),
            direction: direction.into(),
            raw,
            sigma,
        };
        if by_id.insert(id.to_owned(), stream).is_some() {
            return Err(bad(row.line, "duplicate stream"));
        }
    }
    let streams = by_id.into_values().collect::<Vec<_>>();
    if !(2..=16).contains(&streams.len()) {
        return Err(bad(1, "2-16 complete streams required"));
    }
    let positions = streams
        .iter()
        .flat_map(|s| CHANNELS.map(|c| format!("{}/{c}", s.id)))
        .enumerate()
        .map(|(i, k)| (k, i))
        .collect::<BTreeMap<_, _>>();
    let n = positions.len();
    let mut pairs = BTreeMap::new();
    for row in doc.rows.iter().filter(|r| r.table == "correlation") {
        let f = fields(&row.fields, &["left", "right", "value"], &[], row.line)?;
        let left = *positions
            .get(get(&f, "left")?.0)
            .ok_or_else(|| bad(row.line, "unknown left coordinate"))?;
        let right = *positions
            .get(get(&f, "right")?.0)
            .ok_or_else(|| bad(row.line, "unknown right coordinate"))?;
        let value = number(get(&f, "value")?)?;
        if value.abs() > 1.0
            || (left == right && value != 1.0)
            || pairs
                .insert((left.min(right), left.max(right)), value)
                .is_some()
        {
            return Err(bad(row.line, "duplicate or invalid correlation; unit diagonal required"));
        }
    }
    if pairs.len() != n * (n + 1) / 2 {
        return Err(bad(
            1,
            "every unordered raw correlation, including zero cross terms, must be explicit",
        ));
    }
    let mut correlation = vec![0.0; n * n];
    for ((i, j), v) in pairs {
        correlation[i * n + j] = v;
        correlation[j * n + i] = v;
    }
    Ok(Parsed {
        scope, dry_basis, streams, opening, closing, correlation, tolerance, limit, iterations,
    })
}

fn solve(input: &Parsed) -> Result<RawBalanceSummary> {
    let n = input.streams.len() * 3;
    let mut initial = Vec::with_capacity(n);
    let mut observations = Vec::new();
    let mut masses = Vec::new();
    let mut raw_residuals = input.opening;
    for (k, r) in raw_residuals.iter_mut().enumerate() {
        *r = r.checked_sub(input.closing[k]).map_err(arithmetic)?;
    }
    let mut row_scales = [1.0_f64; 3];
    for stream in &input.streams {
        let mass_budget = input.tolerance / (100.0 * n as f64);
        let mass = nominal(stream.raw[0], (stream.sigma[0] * 1e-6).min(mass_budget))?;
        let observed = [
            mass,
            nominal(stream.raw[1], (stream.sigma[1] * 1e-6).min(mass_budget / mass))?,
            nominal(stream.raw[2], (stream.sigma[2] * 1e-6).min(mass_budget / mass))?,
        ];
        observations.push(observed);
        masses.push(mass);
        let physical = amounts(stream.raw)?;
        for k in 0..3 {
            raw_residuals[k] = if stream.direction == "in" {
                raw_residuals[k].checked_add(physical[k])
            } else {
                raw_residuals[k].checked_sub(physical[k])
            }
            .map_err(arithmetic)?;
            let value = physical[k].to_f64();
            initial.push(value / mass);
            row_scales[k] += value.abs();
        }
    }
    let mut targets = Vec::new();
    for (k, scale) in row_scales.iter_mut().enumerate() {
        let opening = nominal(input.opening[k], input.tolerance / 100.0)?;
        let closing = nominal(input.closing[k], input.tolerance / 100.0)?;
        *scale += opening + closing;
        if !scale.is_finite() {
            return Err(fail("constraint scale overflow"));
        }
        let change = input.closing[k].checked_sub(input.opening[k]).map_err(arithmetic)?;
        targets.push(nominal(change, input.tolerance / 100.0)? / *scale);
    }
    let mut constraints = vec![0.0; 3 * n];
    for (i, stream) in input.streams.iter().enumerate() {
        for k in 0..3 {
            let sign = if stream.direction == "in" { 1.0 } else { -1.0 };
            constraints[k * n + 3 * i + k] = sign * masses[i] / row_scales[k];
        }
    }
    let tolerances = row_scales.map(|s| input.tolerance / s);
    let fit = reconcile_nonlinear(
        NonlinearProblem {
            initial: &initial,
            correlation: &input.correlation,
            observation_count: n,
            constraints: &constraints,
            targets: &targets,
            tolerances: &tolerances,
            max_iterations: input.iterations,
        },
        |x, residual, jacobian| {
            jacobian.fill(0.0);
            for (i, stream) in input.streams.iter().enumerate() {
                let at = 3 * i;
                let (d, w, e) = (x[at], x[at + 1], x[at + 2]);
                let total = d + w;
                let sigma = stream.sigma;
                let observed = observations[i];
                residual[at] = (masses[i] * total - observed[0]) / sigma[0];
                residual[at + 1] = (w / total - observed[1]) / sigma[1];
                residual[at + 2] = (e / d - observed[2]) / sigma[2];
                jacobian[at * n + at] = masses[i] / sigma[0];
                jacobian[at * n + at + 1] = masses[i] / sigma[0];
                jacobian[(at + 1) * n + at] = -(w / total) / total / sigma[1];
                jacobian[(at + 1) * n + at + 1] = (d / total) / total / sigma[1];
                jacobian[(at + 2) * n + at] = -(e / d) / d / sigma[2];
                jacobian[(at + 2) * n + at + 2] = 1.0 / d / sigma[2];
            }
            Ok(())
        },
        |x| x.chunks_exact(3).all(|v| v[0] > 0.0 && v[1] >= 0.0 && v[2] >= 0.0 && v[2] <= v[0]),
    )?;
    let mut streams = Vec::new();
    let mut residuals = [0.0; 3];
    for (k, residual) in residuals.iter_mut().enumerate() {
        *residual = input.opening[k].to_f64() - input.closing[k].to_f64();
    }
    for (i, stream) in input.streams.iter().enumerate() {
        let d = fit.estimates[3 * i] * masses[i];
        let w = fit.estimates[3 * i + 1] * masses[i];
        let e = fit.estimates[3 * i + 2] * masses[i];
        let sign = if stream.direction == "in" { 1.0 } else { -1.0 };
        for (k, v) in [d, w, e].iter().enumerate() {
            residuals[k] += sign * v;
        }
        streams.push(ReconciledStream {
            id: stream.id.clone(),
            direction: stream.direction.clone(),
            dry_kg: Kilograms::new(d),
            water_kg: Kilograms::new(w),
            elemental_kg: Kilograms::new(e),
            predicted_wet_kg: Kilograms::new(d + w),
            derived_water_fraction: w / (d + w),
            derived_dry_element_fraction: e / d,
        });
    }
    if residuals
        .iter()
        .any(|r| !r.is_finite() || r.abs() > input.tolerance)
    {
        return Err(fail("independent extensive kg conservation audit failed"));
    }
    let covariance = fit
        .local_covariance
        .iter()
        .enumerate()
        .map(|(at, v)| {
            let physical = (v * masses[(at / n) / 3]) * masses[(at % n) / 3];
            if !physical.is_finite() || (*v != 0.0 && physical == 0.0) {
                return Err(fail("local extensive covariance range loss"));
            }
            Ok(KilogramsSquared::new(physical))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(RawBalanceSummary {
        scope: input.scope.clone(),
        dry_basis: input.dry_basis.clone(),
        fixed_opening_kg: input.opening,
        fixed_closing_kg: input.closing,
        streams,
        raw_residuals_exact_kg: raw_residuals,
        residuals_kg: residuals.map(Kilograms::new),
        local_extensive_covariance: covariance,
        standardized_measurement_residuals: fit.standardized_measurement_residuals,
        standardized_misfit: fit.objective,
        relative_stationarity: fit.relative_stationarity,
        standardized_step: fit.relative_step,
        iterations: fit.iterations,
        backtracks: fit.backtracks,
    })
}

/// Pure byte-bound worksheet admission. Kernel failure retains original bytes
/// and the source Claim. A failed declared misfit screen retains fitted diagnostics
/// but never supplies an admitted reconciliation Claim.
pub fn evaluate(text: &str, source: &SourceAnchor) -> Result<WorksheetResult> {
    if text.len() as u64 > MAX_BYTES
        || sha256(text.as_bytes()) != source.artifact_sha256
        || source.artifact_path.is_empty()
    {
        return Err(bad(1, "source bytes must match the SHA-256 and the 1 MiB limit"));
    }
    let parsed = parse(text)?;
    let tag = Digest32(source.artifact_sha256).to_hex();
    let source = declared(
        text.to_owned(),
        &tag,
        "plant.raw_balance.worksheet/1",
        AdditivityClass::NonAdditive,
        source,
        "complete source-bound raw observations, fixed endpoint stocks, error model and policy".into(),
    );
    let (diagnostics, refusal) = match solve(&parsed) {
        Ok(result) => {
            let refusal = (result.standardized_misfit > parsed.limit).then(|| {
                Refusal::new(
                    RefusalCode::Conservation101,
                    format!(
                        "raw-measurement misfit {} exceeds declared limit {}",
                        result.standardized_misfit, parsed.limit
                    ),
                )
            });
            (Some(result), refusal)
        }
        Err(r) => (None, Some(r)),
    };
    let mut encoder = Encoder::new_frame(*b"RBLN", 1);
    put_source_envelope(&mut encoder, &source, 0);
    encoder.put_str(&diagnostics.as_ref().map_or_else(|| "null".into(), summary_json));
    if let Some(r) = &refusal {
        encoder.put_str(r.code);
        encoder.put_str(&r.what);
    }
    let digest = blake3_hash(&encoder.finish()).to_hex();
    let claim = if refusal.is_none() {
        if let Some(summary) = &diagnostics {
            let mut metadata = Metadata::new();
            metadata.include(&source)?;
            Some(metadata.into_claim("plant.raw_balance.reconciled/1", summary.clone(), &digest))
        } else {
            None
        }
    } else {
        None
    };
    Ok(WorksheetResult { source, parsed, diagnostics, claim, refusal, digest })
}

fn numbers(values: &[f64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| if *v == 0.0 { "0".into() } else { v.to_string() })
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn exacts(values: &[ExactQty]) -> String {
    values.iter().map(|q| exact(*q)).collect::<Vec<_>>().join(",")
}
fn summary_json(s: &RawBalanceSummary) -> String {
    let streams = s.streams.iter().map(|r| {
        format!(
            "{{\"id\":{},\"direction\":{},\"dry_kg\":{},\"water_kg\":{},\"elemental_kg\":{},\"predicted_wet_kg\":{},\"derived_water_fraction\":{},\"derived_dry_element_fraction\":{}}}",
            quoted(&r.id), quoted(&r.direction), r.dry_kg.value(), r.water_kg.value(),
            r.elemental_kg.value(), r.predicted_wet_kg.value(), r.derived_water_fraction,
            r.derived_dry_element_fraction
        )
    }).collect::<Vec<_>>().join(",");
    let scope = format!(
        "{{\"boundary\":{},\"element\":{},\"measurand\":{},\"dry_basis\":{},\"start_ns\":{},\"end_ns\":{}}}",
        quoted(&s.scope.boundary), quoted(&s.scope.element), quoted(&s.scope.measurand),
        quoted(&s.dry_basis), s.scope.interval.start_ns, s.scope.interval.end_ns
    );
    format!(
        "{{\"scope\":{},\"fixed_opening_kg\":[{}],\"fixed_closing_kg\":[{}],\"streams\":[{}],\"raw_residuals_exact_kg\":[{}],\"residuals_kg\":{},\"extensive_order\":\"canonical stream; dry,H2O,element\",\"local_extensive_covariance_kg2\":{},\"standardized_measurement_residuals\":{},\"standardized_misfit\":{},\"relative_stationarity\":{},\"standardized_step\":{},\"iterations\":{},\"backtracks\":{}}}",
        scope, exacts(&s.fixed_opening_kg), exacts(&s.fixed_closing_kg), streams,
        exacts(&s.raw_residuals_exact_kg), numbers(&s.residuals_kg.map(|q| q.value())),
        numbers(&s.local_extensive_covariance.iter().map(|q| q.value()).collect::<Vec<_>>()),
        numbers(&s.standardized_measurement_residuals), s.standardized_misfit,
        s.relative_stationarity, s.standardized_step, s.iterations, s.backtracks
    )
}

#[cfg(test)]
mod tests;
