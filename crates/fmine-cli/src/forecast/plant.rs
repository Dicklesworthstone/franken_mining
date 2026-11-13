//! DESIGN (BD-181): admit conditional plant quantiles through the existing forecast writer.
//! The complete source receipt remains in the hashed operation as admission evidence. Replay verifies
//! those retained bytes and the forecast transitions; it does not re-evaluate the plant calculation.

use std::path::Path;

use fmine_calib::ledger::ForecastMetadata;
use fmine_calib::record::format_utc;
use fmine_claim::forecast_journal::MAX_OPERATION_BYTES;
use fmine_evidence::Refusal;
use fmine_plant::production::ProductionReceipt;

use super::{Row, hex, identifier, invalid, js, time};

const NS_PER_SECOND: i64 = 1_000_000_000;
const EVIDENCE_PREFIX: &str = "# fmine.plant-forecast-admission.v1 ";
const FAMILY: &str = "conditional-plant-production-quantiles/1";
pub const NO_CLAIM: &str = "Conditional plant-model quantiles from declared weighted joint scenarios, not calibrated probabilities or measured production. Only p10, p50 and p90 are registered; interval scoring is available at alpha 0.2, not CRPS or full-distribution scoring. The complete source receipt is retained as admission evidence in the journal operation. Replay verifies the retained bytes, not the original calculation or current source files. Outcomes remain supplied and unverified; registering a prediction neither freezes a cohort nor authorizes operation.";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast journal register-plant --journal <path> --head <hex> --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --id <alias> --target <target> --author-role <role> --time-basis utc-unix-ns --trust-host-clock","targets":["recovery","throughput","dry-tonnes","contained-metal","recovered-metal"],"source":"Admits the complete joint production calculation through its grinding, recovery and explicit grade/exposure worksheets; source IDs, exact scenario weights, physical support, units and element are retained","payload":"Weighted p10, p50 and p90 only; central interval scoring at alpha 0.2, no CRPS or full-distribution scoring","time":"Whole-second UTC Unix-nanosecond physical support is explicitly declared, never inferred or rounded; registration and cohort membership clock upper bounds must precede physical start; supplied outcomes must become available after physical end","journal":"Opt-in operation v9 retains separate registration_before=start and outcome_due=end boundaries and complete source admission evidence; both boundaries are checked on replay; v1-v8 meanings remain unchanged","size":"The complete versioned operation must fit the existing 64 KiB cap; oversized evidence is refused, never truncated","interpretation":"Conditional model outputs and supplied unverified outcomes; no calibration, measured-production or control-authority claim. Replay authenticates retained bytes and transitions, not the original plant calculation or current source files"}"#;

/// A physical target is fixed by the adapter, never inferred from a caller-supplied unit label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Recovery,
    Throughput,
    DryTonnes,
    ContainedMetal,
    RecoveredMetal,
}

impl Target {
    pub fn parse(value: &str) -> Result<Self, Refusal> {
        match value {
            "recovery" => Ok(Self::Recovery),
            "throughput" => Ok(Self::Throughput),
            "dry-tonnes" => Ok(Self::DryTonnes),
            "contained-metal" => Ok(Self::ContainedMetal),
            "recovered-metal" => Ok(Self::RecoveredMetal),
            _ => Err(invalid(
                "plant forecast target must be recovery, throughput, dry-tonnes, contained-metal or recovered-metal",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recovery => "recovery",
            Self::Throughput => "throughput",
            Self::DryTonnes => "dry-tonnes",
            Self::ContainedMetal => "contained-metal",
            Self::RecoveredMetal => "recovered-metal",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Recovery => "fraction",
            Self::Throughput => "dry_metric_tonnes_per_hour",
            Self::DryTonnes => "dry_metric_tonnes",
            Self::ContainedMetal | Self::RecoveredMetal => "kg",
        }
    }

    fn quantity(self, element: &str) -> String {
        match self {
            Self::Recovery => format!("model_recovery_fraction:{element}"),
            Self::Throughput => "dry_feed_throughput".into(),
            Self::DryTonnes => "dry_feed_mass".into(),
            Self::ContainedMetal => format!("contained_feed_metal:{element}"),
            Self::RecoveredMetal => format!("model_recovered_metal:{element}"),
        }
    }

    fn quantiles(self, receipt: &ProductionReceipt) -> [f64; 3] {
        let summary = receipt.summary();
        match self {
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
        }
    }
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
    let [lo, median, hi] = target.quantiles(receipt);
    if [lo, median, hi].iter().any(|v| !v.is_finite() || *v < 0.0)
        || lo > median
        || median > hi
        || (target == Target::Recovery && hi > 1.0)
    {
        return Err(invalid(
            "plant forecast quantiles violate the target's physical domain",
        ));
    }
    hex(receipt.digest())?;
    let metadata = ForecastMetadata {
        family: FAMILY.into(),
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
        evidence_header(
            receipt.digest(),
            target,
            interval.start_ns,
            interval.end_ns,
            [lo, median, hi]
        ),
    );
    // DESIGN: v9 retains two distinct boundaries. Registration and cohort membership precede START;
    // the unchanged core ledger requires outcome availability after END. Neither boundary is rounded.
    let request = format!(
        "{EVIDENCE_PREFIX}{evidence_json}\n[[plant_forecast]]\nid = {}\nmodel_key = {}\nregistration_before = {}\noutcome_due = {}\nkind = \"quantiles\"\nlevels = \"0.1,0.5,0.9\"\nvalues = {}\nfamily = {}\nauthor_role = {}\ndomain = {}\nhorizon = {}\nsite = {}\nquantity = {}\nunit = {}\n",
        js(registration.id),
        js(&format!(
            "plant-production:{}:{}",
            target.as_str(),
            receipt.digest()
        )),
        js(&start),
        js(&end),
        js(&format!("{lo},{median},{hi}")),
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
    })
}

fn evidence_header(digest: &str, target: Target, start: i64, end: i64, values: [f64; 3]) -> String {
    let [lo, median, hi] = values;
    format!(
        "{{\"schema\":\"fmine.plant-forecast-admission.v1\",\"receipt_digest\":{},\"target\":{},\"unit\":{},\"time_basis\":\"utc-unix-ns\",\"physical_start_ns\":{start},\"physical_end_ns\":{end},\"levels\":[0.1,0.5,0.9],\"values\":[{lo},{median},{hi}],\"no_claim\":{},\"source_receipt\":",
        js(digest),
        js(target.as_str()),
        js(target.unit()),
        js(NO_CLAIM),
    )
}

/// Validate v9's typed projection and chronology without claiming to re-evaluate retained source bytes.
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
    if start >= end
        || row.string("family")? != FAMILY
        || row.string("horizon")? != format!("UTC [{start_text},{end_text})")
        || row.string("unit")? != target.unit()
        || row.string("kind")? != "quantiles"
        || row.numbers("levels")? != [0.1, 0.5, 0.9]
    {
        return Err(invalid(
            "plant registration target, support or quantile metadata is inconsistent",
        ));
    }
    let quantity = row.string("quantity")?;
    let expected = match target {
        Target::Throughput | Target::DryTonnes => target.quantity(""),
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
    let values = row.numbers("values")?;
    if values.len() != 3
        || values
            .iter()
            .any(|v| *v < 0.0 || (target == Target::Recovery && *v > 1.0))
        || values.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(invalid(
            "plant registration quantiles violate the target's physical domain",
        ));
    }
    let mut evidence = operation
        .lines()
        .filter_map(|line| line.strip_prefix(EVIDENCE_PREFIX));
    let Some(retained) = evidence.next() else {
        return Err(invalid(
            "plant registration is missing retained admission evidence",
        ));
    };
    let expected = evidence_header(
        digest,
        target,
        start,
        end,
        [values[0], values[1], values[2]],
    );
    if evidence.next().is_some()
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
    presentation.1.push_str(NO_CLAIM);
    presentation.1.push_str(" Registration and cohort membership must precede registration_before, the physical interval's start. Supplied outcomes must become available after outcome_due, the physical interval's end. Both boundaries and the complete target support remain frozen.\n");
    Ok(())
}

#[cfg(test)]
#[path = "../../../xtask/testdata/forecast_plant_controls.rs"]
mod tests;
