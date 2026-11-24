//! DESIGN (BD-181): admit complete weighted plant scenarios through the existing forecast writer.
//! The complete source receipt remains in the hashed operation as admission evidence. Replay verifies
//! those retained bytes and the forecast transitions; it does not re-evaluate the plant calculation.

pub(in crate::forecast) mod outcome;

use std::path::Path;

use fmine_calib::ledger::{ForecastMetadata, ForecastPayload};
use fmine_calib::record::format_utc;
use fmine_claim::forecast_journal::MAX_OPERATION_BYTES;
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::omf::json::{Json, parse as parse_json};
use fmine_plant::production::{ProductionPoint, ProductionReceipt};
use fmine_units::ExactQty;

use super::{Row, hex, identifier, invalid, js, time};

pub const OUTCOME_DOCS_JSON: &str = outcome::ROBOT_DOCS_JSON;

const NS_PER_SECOND: i64 = 1_000_000_000;
const NS_PER_HOUR: i128 = 3_600_000_000_000;
const LEGACY_EVIDENCE_PREFIX: &str = "# fmine.plant-forecast-admission.v1 ";
const LEGACY_FAMILY: &str = "conditional-plant-production-quantiles/1";
const LEGACY_NO_CLAIM: &str = "Conditional plant-model quantiles from declared weighted joint scenarios, not calibrated probabilities or measured production. Only p10, p50 and p90 are registered; interval scoring is available at alpha 0.2, not CRPS or full-distribution scoring. The complete source receipt is retained as admission evidence in the journal operation. Replay verifies the retained bytes, not the original calculation or current source files. Outcomes remain supplied and unverified; registering a prediction neither freezes a cohort nor authorizes operation.";
const EVIDENCE_PREFIX: &str = "# fmine.plant-forecast-admission.v2 ";
const FAMILY: &str = "conditional-plant-production-weighted/2";
pub const NO_CLAIM: &str = "Conditional plant-model distribution from the complete declared weighted joint scenarios, not calibrated probabilities or measured production. Every selected target value and its exact probability are registered without normalization or resampling; CRPS and central interval scoring retain that marginal distribution. Weighted central levels interpret alpha through its canonical shortest decimal spelling, with exact cumulative mass and no interpolation. The complete source receipt and scenario identities remain admission evidence in the journal operation. Replay verifies retained bytes and transitions, not the original calculation or current source files. Outcomes remain supplied and unverified; registering a prediction neither freezes a cohort nor authorizes operation. No joint forecast scoring or calibrated scenario probabilities are inferred.";
pub const VERIFICATION_NO_CLAIM: &str = "Offline recomputation uses the current plant implementation and only the three retained worksheets. Source SHA-256, complete calculated receipt and original forecast projection must match exactly. This is conditional numerical reproduction in the DeterministicPerIsa class, not an independent model oracle, external source audit, calibrated forecast, timestamp proof or plant validation. The journal and its historical interpretation remain unchanged; an unpinned self-consistent replacement history cannot be authenticated by this check.";
pub const VERIFICATION_DOCS_JSON: &str = r#"{"command":"forecast journal verify-plant --journal <path> --forecast <alias> [--head <hex>]","input":"One saved plant registration, weighted v14 or legacy quantile v9; all three exact worksheets and their original source anchors must be retained","verification":"Recompute grinding, recovery and joint production; compare every retained source and result field, receipt digest, scenario identity/order, exact probability, physical support and forecast projection","correction_fit":"An optional source-bound aligned paired recovery fit is recomputed from the retained worksheets. Its evidence_cut_ns must be less than or equal to the earliest possible registration time under the saved record's clock bound. A cut before physical start is insufficient if it follows registration. This check establishes no calibration; absent-fit v9/v14 semantics are unchanged","io":"Read-only journal lock; no clock sample, current worksheet reads or journal writes; retained paths are provenance only","compatibility":"Ordinary replay and durable operation bytes are unchanged. Explicit verification uses current numerical implementations in DeterministicPerIsa class; differences refuse instead of rewriting historical values","limits":"Source/model declarations remain assumed and uncalibrated. No independent oracle, external timestamp or authenticity proof for an unpinned replacement history"}"#;
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"command":"forecast journal register-plant --journal <path> --head <hex> --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --id <alias> --target <target> --author-role <role> --time-basis utc-unix-ns --trust-host-clock","#,
    r#""targets":["recovery","throughput","dry-tonnes","contained-metal","recovered-metal","period-throughput","head-grade"],"#,
    r#""head_grade":"The declared feed grade in grams per dry metric tonne for the selected element. Every weighted scenario must process positive dry feed. Resolve against the original retained production balance","#,
    r#""period_throughput":"Each scenario's dry tonnes divided by the elapsed UTC hours of the complete frozen interval. Scheduled hours must fit inside that interval. This target uses the weighted operation only and can resolve from the retained production balance","#,
    r#""source":"Admits the complete joint production calculation through its grinding, recovery and explicit grade/exposure worksheets; scenario identities and order, exact probabilities, physical support, units and element are retained","#,
    r#""supply_limited_v2":"Dry-tonnes, contained-metal and recovered-metal targets use the explicitly supply-limited production in each state. Throughput retains its operating-capacity meaning; the source receipt separately reports the effective rate over scheduled exposure. V2 feed is available at start and curtailment reduces exposure at the unchanged operating point","#,
    r#""correction_fit":"An optional source-bound aligned paired recovery fit must have evidence_cut_ns less than or equal to the earliest registration clock bound; this is checked before append. verify-plant recomputes the fit from retained worksheets and checks the saved registration bound. The fit establishes no calibration, and absent-fit semantics are unchanged","#,
    r#""payload":"Complete weighted empirical marginal distribution, with every target value and exact positive probability; CRPS and central interval scoring, no normalization or resampling. Weighted interval alpha uses its canonical shortest decimal spelling and exact inverse-CDF comparisons; no interpolation","#,
    r#""time":"Whole-second UTC Unix-nanosecond physical support is explicitly declared, never inferred or rounded; registration and cohort membership clock upper bounds must precede physical start; supplied outcomes must become available after physical end","#,
    r#""journal":"Opt-in operation v14 retains the complete weighted payload, separate registration_before=start and outcome_due=end boundaries and complete source admission evidence; both boundaries are checked on replay; legacy v9 quantile evidence and identities retain their original interpretation","#,
    r#""size":"The complete versioned operation must fit the existing 64 KiB cap; oversized evidence is refused, never truncated","#,
    r#""interpretation":"Conditional model outputs and supplied unverified outcomes; no calibration, measured-production, joint-distribution scoring or control-authority claim. Replay authenticates retained bytes and transitions, not the original plant calculation or current source files"}"#,
);

/// A physical target is fixed by the adapter, never inferred from a caller-supplied unit label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Recovery,
    Throughput,
    PeriodThroughput,
    HeadGrade,
    DryTonnes,
    ContainedMetal,
    RecoveredMetal,
}

impl Target {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "recovery" => Ok(Self::Recovery),
            "throughput" => Ok(Self::Throughput),
            "period-throughput" => Ok(Self::PeriodThroughput),
            "head-grade" => Ok(Self::HeadGrade),
            "dry-tonnes" => Ok(Self::DryTonnes),
            "contained-metal" => Ok(Self::ContainedMetal),
            "recovered-metal" => Ok(Self::RecoveredMetal),
            _ => Err(invalid(
                "plant forecast target must be recovery, throughput, period-throughput, head-grade, dry-tonnes, contained-metal or recovered-metal",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recovery => "recovery",
            Self::Throughput => "throughput",
            Self::PeriodThroughput => "period-throughput",
            Self::HeadGrade => "head-grade",
            Self::DryTonnes => "dry-tonnes",
            Self::ContainedMetal => "contained-metal",
            Self::RecoveredMetal => "recovered-metal",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Recovery => "fraction",
            Self::Throughput | Self::PeriodThroughput => "dry_metric_tonnes_per_hour",
            Self::HeadGrade => "g_per_dry_t",
            Self::DryTonnes => "dry_metric_tonnes",
            Self::ContainedMetal | Self::RecoveredMetal => "kg",
        }
    }

    fn quantity(self, element: &str) -> String {
        match self {
            Self::Recovery => format!("model_recovery_fraction:{element}"),
            Self::Throughput => "dry_feed_throughput".into(),
            Self::PeriodThroughput => "dry_feed_tonnes_per_elapsed_utc_hour".into(),
            Self::HeadGrade => format!("dry_feed_head_grade:{element}"),
            Self::DryTonnes => "dry_feed_mass".into(),
            Self::ContainedMetal => format!("contained_feed_metal:{element}"),
            Self::RecoveredMetal => format!("model_recovered_metal:{element}"),
        }
    }

    fn sample(self, point: &ProductionPoint, elapsed: ExactQty) -> Result<f64, Refusal> {
        Ok(match self {
            Self::Recovery => point.recovery_fraction.value(),
            Self::Throughput => point.throughput.value(),
            Self::PeriodThroughput => {
                let dry = point.dry_tonnes.value();
                let rate = dry / elapsed.to_f64();
                if !rate.is_finite() || (dry > 0.0 && rate == 0.0) {
                    return Err(Refusal::new(
                        RefusalCode::NumericalCertificateFailed207,
                        "period throughput loses binary64 range",
                    ));
                }
                rate
            }
            Self::HeadGrade => {
                if point.dry_tonnes.value() == 0.0 {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "head grade is undefined for scenario {} with zero processed dry feed",
                            point.scenario_id
                        ),
                    ));
                }
                point.grade_g_per_dry_t.to_f64()
            }
            Self::DryTonnes => point.dry_tonnes.value(),
            Self::ContainedMetal => point.contained_metal.value(),
            Self::RecoveredMetal => point.recovered_metal.value(),
        })
    }

    fn quantiles(self, receipt: &ProductionReceipt) -> Result<[f64; 3], Refusal> {
        let summary = receipt.summary();
        Ok(match self {
            Self::Recovery => [
                summary.recovery.p10.value(),
                summary.recovery.p50.value(),
                summary.recovery.p90.value(),
            ],
            Self::Throughput => [
                summary.throughput.p10.value(),
                summary.throughput.p50.value(),
                summary.throughput.p90.value(),
            ],
            Self::PeriodThroughput | Self::HeadGrade => {
                return Err(self.weighted_projection_required());
            }
            Self::DryTonnes => [
                summary.dry_tonnes.p10.value(),
                summary.dry_tonnes.p50.value(),
                summary.dry_tonnes.p90.value(),
            ],
            Self::ContainedMetal => [
                summary.contained_metal.p10.value(),
                summary.contained_metal.p50.value(),
                summary.contained_metal.p90.value(),
            ],
            Self::RecoveredMetal => [
                summary.recovered_metal.p10.value(),
                summary.recovered_metal.p50.value(),
                summary.recovered_metal.p90.value(),
            ],
        })
    }

    fn weighted_projection_required(self) -> Refusal {
        invalid(format!(
            "{} requires a weighted plant forecast",
            self.as_str()
        ))
    }
}

fn elapsed_hours(start_ns: i64, end_ns: i64) -> Result<ExactQty, Refusal> {
    if start_ns >= end_ns {
        return Err(invalid(
            "plant forecast needs a positive elapsed UTC interval",
        ));
    }
    ExactQty::new(i128::from(end_ns) - i128::from(start_ns), NS_PER_HOUR)
        .map_err(|_| invalid("cannot represent elapsed plant hours"))
}

pub(super) struct Registration<'a> {
    pub input: &'a Path,
    pub grinding: &'a Path,
    pub recovery: &'a Path,
    pub id: &'a str,
    pub target: Target,
    pub author_role: &'a str,
    pub time_basis: &'a str,
}

pub(super) struct Prepared {
    pub request: String,
    pub evidence_json: String,
    pub fit_evidence_cut_ns: Option<i64>,
}

/// A fitted correction can enter a prospective forecast only after all declared fitting evidence
/// is available, even at the earliest registration time allowed by the stored clock bound.
pub(super) fn validate_fit_evidence_time(
    evidence_cut_ns: Option<i64>,
    registration_earliest_ns: i64,
) -> Result<(), Refusal> {
    if evidence_cut_ns.is_some_and(|cut| cut > registration_earliest_ns) {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "plant correction fit evidence cut must not exceed the registration clock's earliest possible time",
        ));
    }
    Ok(())
}

fn fit_evidence_cut(receipt: &ProductionReceipt) -> Option<i64> {
    receipt
        .recovery()
        .model()
        .correction()
        .value
        .fit_validity
        .as_ref()
        .map(|validity| validity.evidence_cut_ns)
}

pub(super) fn prepare(registration: &Registration<'_>, root: &Path) -> Result<Prepared, Refusal> {
    if registration.time_basis != "utc-unix-ns" {
        return Err(invalid(
            "plant forecast admission requires the explicit time basis utc-unix-ns",
        ));
    }
    identifier(registration.id)?;
    let loaded = crate::plant::production::load(
        registration.input,
        registration.grinding,
        registration.recovery,
        root,
    )?;
    from_receipt(registration, loaded.receipt(), &loaded.to_json())
}

fn from_receipt(
    registration: &Registration<'_>,
    receipt: &ProductionReceipt,
    source_json: &str,
) -> Result<Prepared, Refusal> {
    let interval = receipt.summary().grinding_scope.interval;
    // The production receipt has already admitted each exact schedule against this interval.
    let elapsed = elapsed_hours(interval.start_ns, interval.end_ns)?;
    let payload = ForecastPayload::WeightedEmpirical {
        samples: receipt
            .points()
            .iter()
            .map(|point| registration.target.sample(point, elapsed))
            .collect::<Result<_, _>>()?,
        weights: receipt.points().iter().map(|point| point.weight).collect(),
    };
    from_projection(registration, receipt, source_json, payload)
}

/// Preserve the original quantile projection for explicit verification and durable replay controls.
fn legacy_from_receipt(
    registration: &Registration<'_>,
    receipt: &ProductionReceipt,
    source_json: &str,
) -> Result<Prepared, Refusal> {
    let payload = ForecastPayload::Quantiles {
        levels: vec![0.1, 0.5, 0.9],
        values: registration.target.quantiles(receipt)?.to_vec(),
    };
    from_projection(registration, receipt, source_json, payload)
}

fn from_projection(
    registration: &Registration<'_>,
    receipt: &ProductionReceipt,
    source_json: &str,
    payload: ForecastPayload,
) -> Result<Prepared, Refusal> {
    let summary = receipt.summary();
    let scope = &summary.grinding_scope;
    let interval = scope.interval;
    if interval.start_ns >= interval.end_ns
        || interval.start_ns.rem_euclid(NS_PER_SECOND) != 0
        || interval.end_ns.rem_euclid(NS_PER_SECOND) != 0
    {
        return Err(invalid(
            "plant forecast UTC support must be a positive interval on whole-second boundaries; no clock-axis conversion or rounding is inferred",
        ));
    }
    let start = format_utc(interval.start_ns / NS_PER_SECOND);
    let end = format_utc(interval.end_ns / NS_PER_SECOND);
    if time(&start)? != interval.start_ns || time(&end)? != interval.end_ns {
        return Err(invalid(
            "plant forecast support cannot round-trip through UTC",
        ));
    }
    let target = registration.target;
    validate_projection(target, &payload)?;
    let (prefix, family, payload_fields) = match &payload {
        ForecastPayload::WeightedEmpirical { samples, weights } => (
            EVIDENCE_PREFIX,
            FAMILY,
            format!(
                "kind = \"weighted_empirical\"\nsamples = {}\nweights = {}\n",
                js(&samples
                    .iter()
                    .map(f64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")),
                js(&weights
                    .iter()
                    .map(|weight| format!("{}/{}", weight.num(), weight.den()))
                    .collect::<Vec<_>>()
                    .join(",")),
            ),
        ),
        ForecastPayload::Quantiles { values, .. } => (
            LEGACY_EVIDENCE_PREFIX,
            LEGACY_FAMILY,
            format!(
                "kind = \"quantiles\"\nlevels = \"0.1,0.5,0.9\"\nvalues = {}\n",
                js(&values
                    .iter()
                    .map(f64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")),
            ),
        ),
        _ => return Err(invalid("unsupported plant forecast projection")),
    };
    hex(receipt.digest())?;
    let metadata = ForecastMetadata {
        family: family.into(),
        author_role: registration.author_role.into(),
        domain: format!("{}/{}", scope.circuit, scope.blend),
        horizon: format!("UTC [{start},{end})"),
        site: scope.boundary.clone(),
        quantity: target.quantity(&summary.recovery_scope.element),
        unit: target.unit().into(),
    };
    metadata.validate()?;
    if source_json.contains(['\n', '\r'])
        || !source_json.starts_with('{')
        || !source_json.ends_with('}')
    {
        return Err(invalid(
            "plant admission evidence requires one complete compact JSON object",
        ));
    }
    let evidence_json = format!(
        "{}{source_json}}}",
        projection_header(
            receipt.digest(),
            target,
            interval.start_ns,
            interval.end_ns,
            &payload,
        )?,
    );
    // DESIGN: both versions retain distinct boundaries. Registration and cohort membership precede START;
    // the unchanged core ledger requires outcome availability after END. Neither boundary is rounded.
    let request = format!(
        "{prefix}{evidence_json}\n[[plant_forecast]]\nid = {}\nmodel_key = {}\nregistration_before = {}\noutcome_due = {}\n{payload_fields}family = {}\nauthor_role = {}\ndomain = {}\nhorizon = {}\nsite = {}\nquantity = {}\nunit = {}\n",
        js(registration.id),
        js(&format!(
            "plant-production:{}:{}",
            target.as_str(),
            receipt.digest()
        )),
        js(&start),
        js(&end),
        js(&metadata.family),
        js(&metadata.author_role),
        js(&metadata.domain),
        js(&metadata.horizon),
        js(&metadata.site),
        js(&metadata.quantity),
        js(&metadata.unit),
    );
    if request.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "complete plant forecast admission exceeds the 64 KiB journal operation limit; evidence is never dropped or truncated",
        ));
    }
    Ok(Prepared {
        request,
        evidence_json,
        fit_evidence_cut_ns: fit_evidence_cut(receipt),
    })
}

fn validate_projection(target: Target, payload: &ForecastPayload) -> Result<(), Refusal> {
    let values = match payload {
        ForecastPayload::WeightedEmpirical { samples, weights } => {
            if samples.is_empty() || samples.len() != weights.len() {
                return Err(invalid(
                    "plant forecast must retain every scenario and its probability",
                ));
            }
            samples
        }
        ForecastPayload::Quantiles { levels, values } => {
            if matches!(target, Target::PeriodThroughput | Target::HeadGrade) {
                return Err(target.weighted_projection_required());
            }
            if levels != &[0.1, 0.5, 0.9]
                || values.len() != 3
                || values.windows(2).any(|pair| pair[0] > pair[1])
            {
                return Err(invalid(
                    "legacy plant forecast requires its original p10/p50/p90 projection",
                ));
            }
            values
        }
        _ => return Err(invalid("unsupported plant forecast projection")),
    };
    if values.iter().any(|value| {
        !value.is_finite()
            || *value < 0.0
            || (target == Target::Recovery && *value > 1.0)
            || (target == Target::HeadGrade && *value > 1_000_000.0)
    }) {
        return Err(invalid(
            "plant forecast values violate the target's physical domain",
        ));
    }
    Ok(())
}

fn legacy_evidence_header(
    digest: &str,
    target: Target,
    start: i64,
    end: i64,
    values: [f64; 3],
) -> String {
    let [lo, median, hi] = values;
    format!(
        "{{\"schema\":\"fmine.plant-forecast-admission.v1\",\"receipt_digest\":{},\"target\":{},\"unit\":{},\"time_basis\":\"utc-unix-ns\",\"physical_start_ns\":{start},\"physical_end_ns\":{end},\"levels\":[0.1,0.5,0.9],\"values\":[{lo},{median},{hi}],\"no_claim\":{},\"source_receipt\":",
        js(digest),
        js(target.as_str()),
        js(target.unit()),
        js(LEGACY_NO_CLAIM),
    )
}

fn projection_header(
    digest: &str,
    target: Target,
    start: i64,
    end: i64,
    payload: &ForecastPayload,
) -> Result<String, Refusal> {
    match payload {
        ForecastPayload::WeightedEmpirical { .. } => Ok(format!(
            "{{\"schema\":\"fmine.plant-forecast-admission.v2\",\"receipt_digest\":{},\"target\":{},\"unit\":{},\"time_basis\":\"utc-unix-ns\",\"physical_start_ns\":{start},\"physical_end_ns\":{end},\"payload\":{},\"no_claim\":{},\"source_receipt\":",
            js(digest),
            js(target.as_str()),
            js(target.unit()),
            super::payload_json(payload),
            js(NO_CLAIM),
        )),
        ForecastPayload::Quantiles { values, .. } => {
            let values: [f64; 3] = values
                .as_slice()
                .try_into()
                .map_err(|_| invalid("legacy plant projection requires three quantiles"))?;
            Ok(legacy_evidence_header(digest, target, start, end, values))
        }
        _ => Err(invalid("unsupported plant forecast projection")),
    }
}

/// Validate legacy v9 or weighted v14 projection and chronology without re-evaluating retained source bytes.
pub(super) fn registration_start(row: &Row<'_>, operation: &str) -> Result<i64, Refusal> {
    let model = row
        .string("model_key")?
        .strip_prefix("plant-production:")
        .ok_or_else(|| invalid("plant registration requires its receipt-bound model key"))?;
    let (target, digest) = model
        .split_once(':')
        .ok_or_else(|| invalid("plant model key must retain target and receipt digest"))?;
    let target = Target::parse(target)?;
    hex(digest)?;
    let start_text = row.string("registration_before")?;
    let end_text = row.string("outcome_due")?;
    let start = time(start_text)?;
    let end = time(end_text)?;
    let (prefix, family, payload) = match row.string("kind")? {
        "quantiles" => (
            LEGACY_EVIDENCE_PREFIX,
            LEGACY_FAMILY,
            ForecastPayload::Quantiles {
                levels: row.numbers("levels")?,
                values: row.numbers("values")?,
            },
        ),
        "weighted_empirical" => (
            EVIDENCE_PREFIX,
            FAMILY,
            ForecastPayload::WeightedEmpirical {
                samples: row.numbers("samples")?,
                weights: row.exact_weights()?,
            },
        ),
        _ => {
            return Err(invalid(
                "plant registration requires a supported source projection",
            ));
        }
    };
    if start >= end
        || row.string("family")? != family
        || row.string("horizon")? != format!("UTC [{start_text},{end_text})")
        || row.string("unit")? != target.unit()
    {
        return Err(invalid(
            "plant registration target, support or projection metadata is inconsistent",
        ));
    }
    let quantity = row.string("quantity")?;
    let expected = match target {
        Target::Throughput | Target::PeriodThroughput | Target::DryTonnes => target.quantity(""),
        _ => {
            let prefix = target.quantity("");
            let element = quantity
                .strip_prefix(&prefix)
                .filter(|element| !element.is_empty())
                .ok_or_else(|| invalid("plant metal target must retain its declared element"))?;
            target.quantity(element)
        }
    };
    if quantity != expected {
        return Err(invalid(
            "plant registration quantity does not match its target",
        ));
    }
    validate_projection(target, &payload)?;
    let mut evidence = operation.lines().filter_map(|line| {
        [LEGACY_EVIDENCE_PREFIX, EVIDENCE_PREFIX]
            .into_iter()
            .find_map(|candidate| line.strip_prefix(candidate).map(|value| (candidate, value)))
    });
    let Some((retained_prefix, retained)) = evidence.next() else {
        return Err(invalid(
            "plant registration is missing retained admission evidence",
        ));
    };
    let expected = projection_header(digest, target, start, end, &payload)?;
    if retained_prefix != prefix
        || evidence.next().is_some()
        || retained
            .strip_prefix(&expected)
            .is_none_or(|source| !source.starts_with('{') || !source.ends_with("}}"))
    {
        return Err(invalid(
            "plant registration requires complete admission evidence matching its typed projection",
        ));
    }
    Ok(start)
}

fn retained_field<'a>(value: &'a Json, name: &str) -> Result<&'a Json, Refusal> {
    value
        .get(name)
        .ok_or_else(|| invalid(format!("plant verification requires retained {name}")))
}

fn retained_text<'a>(value: &'a Json, name: &str) -> Result<&'a str, Refusal> {
    retained_field(value, name)?
        .as_str()
        .ok_or_else(|| invalid(format!("plant verification requires text for {name}")))
}

fn snapshot(value: &Json, input: bool) -> Result<(String, SourceAnchor), Refusal> {
    let text = retained_text(value, "submitted_worksheet")?;
    let source = retained_field(value, "source")?;
    let locator = retained_text(if input { source } else { value }, "locator")?;
    if !input && retained_field(source, "bytes")?.u64() != Some(text.len() as u64) {
        return Err(invalid(
            "plant verification source length differs from retained bytes",
        ));
    }
    Ok((
        text.to_string(),
        SourceAnchor::new(
            retained_text(source, "path")?,
            hex(retained_text(source, "sha256")?)?.0,
            locator,
        ),
    ))
}

/// DESIGN: opt-in numerical replay never changes historical v9/v14 admission semantics. The complete
/// machine-generated receipt is compared, so a valid hash chain cannot lend provenance to an altered
/// displayed result, replacement scenario population or invented forecast distribution.
pub(super) fn verify(
    row: &Row<'_>,
    operation: &str,
    registration_earliest_ns: i64,
) -> Result<(String, String), Refusal> {
    if row.0.table != "plant_forecast" || operation.len() > MAX_OPERATION_BYTES {
        return Err(invalid(
            "plant verification requires one bounded plant registration",
        ));
    }
    registration_start(row, operation)?;
    let evidence = operation
        .lines()
        .find_map(|line| {
            line.strip_prefix(EVIDENCE_PREFIX)
                .or_else(|| line.strip_prefix(LEGACY_EVIDENCE_PREFIX))
        })
        .ok_or_else(|| invalid("plant admission evidence missing"))?;
    let retained = parse_json(evidence.as_bytes()).map_err(invalid)?;
    let source = retained_field(&retained, "source_receipt")?;
    let sources = retained_field(source, "sources")?;
    let loaded = crate::plant::production::from_snapshots(
        snapshot(retained_field(source, "production")?, true)?,
        snapshot(retained_field(sources, "grinding")?, false)?,
        snapshot(retained_field(sources, "recovery")?, false)?,
    )?;
    validate_fit_evidence_time(fit_evidence_cut(loaded.receipt()), registration_earliest_ns)?;
    let recalculated = loaded.to_json();
    let mismatch = |what| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "retained plant {what} does not reproduce under the current numerical implementation"
            ),
        )
    };
    if parse_json(recalculated.as_bytes()).map_err(invalid)? != *source {
        return Err(mismatch("source/result receipt"));
    }
    let registration = Registration {
        input: Path::new(""),
        grinding: Path::new(""),
        recovery: Path::new(""),
        id: row.string("id")?,
        target: Target::parse(retained_text(&retained, "target")?)?,
        author_role: row.string("author_role")?,
        time_basis: "utc-unix-ns",
    };
    let expected = if row.string("kind")? == "quantiles" {
        legacy_from_receipt(&registration, loaded.receipt(), &recalculated)?
    } else {
        from_receipt(&registration, loaded.receipt(), &recalculated)?
    };
    let document = super::parse_subset(&expected.request).map_err(|e| invalid(e.to_string()))?;
    let expected_row = document
        .rows
        .first()
        .ok_or_else(|| invalid("recomputed plant registration missing"))?;
    if row.0.fields.len() != expected_row.fields.len()
        || expected_row.fields.iter().any(|field| {
            row.0
                .get(&field.key)
                .is_none_or(|saved| saved.value != field.value)
        })
        || parse_json(expected.evidence_json.as_bytes()).map_err(invalid)? != retained
    {
        return Err(mismatch("forecast projection or metadata"));
    }
    let receipt = loaded.receipt();
    let json = format!(
        "{{\"schema\":\"fmine.plant-forecast-verification.v1\",\"status\":\"verified\",\"forecast\":{},\"target\":{},\"receipt_digest\":{},\"scenario_count\":{},\"source_files_opened\":false,\"determinism\":\"DeterministicPerIsa\",\"admission\":{},\"no_claim\":{}}}",
        js(registration.id),
        js(registration.target.as_str()),
        js(receipt.digest()),
        receipt.points().len(),
        expected.evidence_json,
        js(VERIFICATION_NO_CLAIM),
    );
    let markdown = format!(
        "# Plant forecast numerical verification\n\nVerified all three retained source worksheets, the complete production receipt and the original forecast projection.\n\nForecast: {}. Target: {}. Scenarios: {}. Receipt: `{}`.\n\n{VERIFICATION_NO_CLAIM}\n\n{}",
        js(registration.id),
        registration.target.as_str(),
        receipt.points().len(),
        receipt.digest(),
        loaded.to_markdown(),
    );
    Ok((json, markdown))
}

/// Render before the append is committed, so a presentation refusal cannot follow a successful write.
pub(super) fn attach_evidence(
    presentation: &mut (String, String),
    evidence: &str,
) -> Result<(), Refusal> {
    let prefix = presentation
        .0
        .strip_suffix('}')
        .ok_or_else(|| invalid("invalid forecast journal presentation"))?;
    presentation.0 = format!("{prefix},\"plant_admission\":{evidence}}}");
    presentation
        .1
        .push_str("\n## Conditional plant forecast\n\n");
    presentation.1.push_str(
        if evidence.starts_with("{\"schema\":\"fmine.plant-forecast-admission.v1\",") {
            LEGACY_NO_CLAIM
        } else {
            NO_CLAIM
        },
    );
    presentation.1.push_str(" Registration and cohort membership must precede registration_before, the physical interval's start. Supplied outcomes must become available after outcome_due, the physical interval's end. Both boundaries and the complete target support remain frozen.\n");
    Ok(())
}

#[cfg(test)]
#[path = "../../../xtask/testdata/forecast_plant_controls.rs"]
mod tests;
