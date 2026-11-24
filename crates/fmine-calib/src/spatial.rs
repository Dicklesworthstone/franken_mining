//! DESIGN (BD-311): source-bound replay of an explicitly declared spatial-front monitor.
//! Determinism: DeterministicPerIsa
//! Worksheet chronology is a declaration, not independent evidence of prospective registration.
//! All registration and availability times are nonnegative UTC Unix nanoseconds.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_claim::{
    AdditivityClass, Claim, ClaimId, Confidentiality, Representation, SpatialSupport,
};
use fmine_core::{Digest32, Encoder, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};
use fmine_units::Qty;

use super::{SpatialFrontPitMonitor, SpatialSample, blake3_hash};

pub const SCHEMA: &str = "fmine.spatial-monitor.v1";
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_OBSERVATIONS: usize = 4096;
pub const METHOD: &str = "Nearest-predecessor Vecchia Gaussian conditioning; separate location and centered-dispersion betting processes, each using alpha/2; first threshold crossing retained.";
pub const NO_CLAIM: &str = "Conditional replay of a declared spatial Gaussian model and supplied observations. All inputs and outputs remain Assumed/Internal. The worksheet does not authenticate observation provenance, policy preregistration, availability times, covariance validity or a common physical support. PIT values and e-process wealth are not probabilities that a model is correct. No predictive calibration, causal diagnosis, automatic retraining, InvestigationCase, decision authority or external time proof is supplied.";

type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone)]
struct Policy {
    model_id: String,
    target_unit: String,
    coordinate_frame: String,
    support: String,
    nugget_var: f64,
    sill_var: f64,
    range_m: f64,
    max_neighbors: usize,
    alpha: f64,
    registered_ns: i64,
}

/// Source-declared observation in the policy's explicit target unit and metre frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub id: String,
    pub available_ns: i64,
    pub prediction_registered_ns: i64,
    pub sample: SpatialSample,
    pub declared_locator: String,
}

/// Diagnostic quantities are exported only inside their source-bound Claim.
#[derive(Debug, Clone, PartialEq)]
pub struct SpatialRow {
    pub sequence: usize,
    pub observation: Observation,
    pub conditional_mean: f64,
    pub conditional_std_dev: f64,
    pub standardized_residual: f64,
    pub pit: f64,
    pub location_log_wealth: f64,
    pub location_max_log_wealth: f64,
    pub dispersion_log_wealth: f64,
    pub dispersion_max_log_wealth: f64,
    pub log_threshold: f64,
    pub location_alarm: bool,
    pub dispersion_alarm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarm {
    pub sequence: usize,
    pub observation_id: String,
    pub available_ns: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpatialSummary {
    pub observations: usize,
    pub first_alarm: Option<Alarm>,
    pub location_alarm: bool,
    pub dispersion_alarm: bool,
}

/// Immutable admitted evidence, ordered diagnostic Claims and their bound summary.
#[derive(Debug, Clone)]
pub struct SpatialReceipt {
    source: SourceAnchor,
    policy: Policy,
    inputs: Vec<Claim<String>>,
    rows: Vec<Claim<SpatialRow>>,
    claim: Claim<SpatialSummary>,
    digest: String,
}

fn invalid(line: usize, detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("spatial monitor line {line}: {}", detail.as_ref()),
    )
}

fn numerical(detail: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("spatial monitor: {}", detail.as_ref()),
    )
}

fn named(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

fn fields(items: &[TomlField], required: &[&str], line: usize) -> Result<()> {
    for item in items {
        if !required.contains(&item.key.as_str()) {
            return Err(invalid(item.line, format!("unknown field {}", item.key)));
        }
    }
    for key in required {
        field(items, key).map_err(|_| invalid(line, format!("missing {key}")))?;
    }
    Ok(())
}

fn field<'a>(items: &'a [TomlField], key: &str) -> Result<&'a TomlField> {
    items
        .iter()
        .find(|item| item.key == key)
        .ok_or_else(|| invalid(1, format!("missing {key}")))
}

fn text<'a>(items: &'a [TomlField], key: &str) -> Result<&'a str> {
    let item = field(items, key)?;
    match &item.value {
        TomlValue::Str(value) if named(value) => Ok(value),
        _ => Err(invalid(
            item.line,
            format!("{key} must be bounded nonempty text"),
        )),
    }
}

fn integer(items: &[TomlField], key: &str) -> Result<i64> {
    let item = field(items, key)?;
    match item.value {
        TomlValue::Int(value) if value >= 0 => Ok(value),
        _ => Err(invalid(
            item.line,
            format!("{key} must be a nonnegative integer"),
        )),
    }
}

fn number(items: &[TomlField], key: &str) -> Result<f64> {
    let item = field(items, key)?;
    let parsed = text(items, key)?.parse::<f64>().ok();
    parsed
        .filter(|value| value.is_finite())
        .ok_or_else(|| invalid(item.line, format!("{key} must be a finite quoted number")))
}

fn support(name: &str) -> Result<SpatialSupport> {
    match name {
        "mining_polygon" => Ok(SpatialSupport::MiningPolygon),
        "point_core" => Ok(SpatialSupport::PointCore),
        _ => Err(invalid(1, "support must be mining_polygon or point_core")),
    }
}

fn policy(items: &[TomlField]) -> Result<Policy> {
    fields(
        items,
        &[
            "schema",
            "model_id",
            "target_unit",
            "coordinate_frame",
            "support",
            "nugget_var",
            "sill_var",
            "range_m",
            "max_neighbors",
            "alpha",
            "registered_ns",
        ],
        1,
    )?;
    if text(items, "schema")? != SCHEMA {
        return Err(invalid(1, format!("schema must be {SCHEMA}")));
    }
    let result = Policy {
        model_id: text(items, "model_id")?.into(),
        target_unit: text(items, "target_unit")?.into(),
        coordinate_frame: text(items, "coordinate_frame")?.into(),
        support: text(items, "support")?.into(),
        nugget_var: number(items, "nugget_var")?,
        sill_var: number(items, "sill_var")?,
        range_m: number(items, "range_m")?,
        max_neighbors: usize::try_from(integer(items, "max_neighbors")?)
            .map_err(|_| invalid(1, "max_neighbors exceeds capacity"))?,
        alpha: number(items, "alpha")?,
        registered_ns: integer(items, "registered_ns")?,
    };
    support(&result.support)?;
    if !(result.nugget_var + result.sill_var).is_finite() {
        return Err(numerical("total variance is not representable"));
    }
    Ok(result)
}

fn observation(items: &[TomlField], line: usize, policy: &Policy) -> Result<Observation> {
    fields(
        items,
        &[
            "id",
            "available_ns",
            "prediction_registered_ns",
            "x_m",
            "y_m",
            "z_m",
            "prior_mean",
            "observed_value",
            "target_unit",
            "coordinate_frame",
            "support",
            "locator",
        ],
        line,
    )?;
    for (key, expected) in [
        ("target_unit", policy.target_unit.as_str()),
        ("coordinate_frame", policy.coordinate_frame.as_str()),
        ("support", policy.support.as_str()),
    ] {
        if text(items, key)? != expected {
            return Err(Refusal::new(
                RefusalCode::SupportMismatch102,
                format!(
                    "spatial monitor line {line}: {key} differs from the policy; no conversion is implicit"
                ),
            ));
        }
    }
    let observation = Observation {
        id: text(items, "id")?.into(),
        available_ns: integer(items, "available_ns")?,
        prediction_registered_ns: integer(items, "prediction_registered_ns")?,
        sample: SpatialSample {
            x: number(items, "x_m")?,
            y: number(items, "y_m")?,
            z: number(items, "z_m")?,
            prior_mean: number(items, "prior_mean")?,
            observed_value: number(items, "observed_value")?,
        },
        declared_locator: text(items, "locator")?.into(),
    };
    if observation.prediction_registered_ns < policy.registered_ns
        || observation.prediction_registered_ns >= observation.available_ns
    {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            format!(
                "spatial monitor line {line}: policy <= prediction registration < outcome availability is required"
            ),
        ));
    }
    Ok(observation)
}

fn anchored(source: &SourceAnchor, locator: &str) -> SourceAnchor {
    SourceAnchor::new(
        &source.artifact_path,
        source.artifact_sha256,
        format!("{}; {locator}", source.locator),
    )
}

fn source_claim<T>(
    metric: &str,
    value: T,
    tag: &str,
    support: SpatialSupport,
    parents: Vec<ClaimId>,
    anchors: Vec<SourceAnchor>,
) -> Claim<T> {
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        support,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        Vec::new(),
        parents,
        anchors,
    )
}

/// Parse supplied bytes and replay their declared order without IO or external authority.
/// Admission and calculation are transactional: an unusable row refuses the complete receipt.
pub fn evaluate(text: &str, source: SourceAnchor) -> Result<SpatialReceipt> {
    if text.len() as u64 > MAX_BYTES {
        return Err(invalid(1, "worksheet exceeds four MiB"));
    }
    if !named(&source.artifact_path) || !named(&source.locator) {
        return Err(invalid(
            1,
            "worksheet needs a bounded source path and locator",
        ));
    }
    if source.artifact_sha256 == [0; 32] || sha256(text.as_bytes()).0 != source.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "spatial monitor worksheet bytes differ from the source SHA-256",
        ));
    }
    let document = parse_subset(text).map_err(|error| invalid(error.line, error.reason))?;
    if document.rows.is_empty() || document.rows.len() > MAX_OBSERVATIONS {
        return Err(invalid(1, "expected one through 4096 observations"));
    }
    let policy = policy(&document.top)?;
    let physical_support = support(&policy.support)?;
    let mut monitor = SpatialFrontPitMonitor::new(
        policy.nugget_var,
        policy.sill_var,
        Qty::new(policy.range_m),
        policy.max_neighbors,
        policy.alpha,
    )
    .map_err(|error| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("spatial monitor policy: {error}"),
        )
    })?;
    let mut observations = Vec::with_capacity(document.rows.len());
    let mut ids = BTreeSet::new();
    let mut previous_time = policy.registered_ns;
    for row in &document.rows {
        if row.table != "observation" {
            return Err(invalid(row.line, format!("unknown table {}", row.table)));
        }
        let value = observation(&row.fields, row.line, &policy)?;
        if !ids.insert(value.id.clone()) {
            return Err(invalid(
                row.line,
                format!("duplicate observation {}", value.id),
            ));
        }
        if value.available_ns <= previous_time {
            return Err(Refusal::new(
                RefusalCode::LateRegistration206,
                format!(
                    "spatial monitor line {}: availability must strictly increase in the declared order",
                    row.line
                ),
            ));
        }
        previous_time = value.available_ns;
        observations.push((value, row.line));
    }
    let policy_value = policy_json(&policy);
    let policy_tag = blake3_hash(policy_value.as_bytes()).to_hex();
    let policy_anchor = anchored(&source, "policy");
    let mut inputs = vec![source_claim(
        "spatial.monitor.policy/1",
        policy_value,
        &policy_tag,
        SpatialSupport::PeriodAggregate,
        Vec::new(),
        vec![policy_anchor.clone()],
    )];
    let mut rows: Vec<Claim<SpatialRow>> = Vec::with_capacity(observations.len());
    let mut first_alarm = None;
    for (observation, line) in observations {
        let sample = observation.sample;
        for prior in &monitor.history {
            let dx = sample.x - prior.x;
            let dy = sample.y - prior.y;
            let dz = sample.z - prior.z;
            if !(dx * dx + dy * dy + dz * dz).is_finite() {
                return Err(numerical("squared spatial distance is not representable"));
            }
        }
        let (conditional_mean, conditional_std_dev) =
            monitor.predictive(sample.x, sample.y, sample.z, sample.prior_mean)?;
        let standardized_residual =
            (sample.observed_value - conditional_mean) / conditional_std_dev;
        if !conditional_mean.is_finite()
            || !conditional_std_dev.is_finite()
            || conditional_std_dev <= 0.0
            || !standardized_residual.is_finite()
        {
            return Err(numerical(
                "conditional distribution or residual is not representable",
            ));
        }
        let input_json = observation_json(&observation, &policy);
        let input_tag = blake3_hash(input_json.as_bytes()).to_hex();
        let observation_anchor = anchored(&source, &format!("observation at line {line}"));
        let input = source_claim(
            "spatial.monitor.observation/1",
            input_json,
            &input_tag,
            physical_support,
            Vec::new(),
            vec![observation_anchor.clone()],
        );
        let (pit, alarm) = monitor.observe_sample(sample)?;
        let value = SpatialRow {
            sequence: rows.len() + 1,
            observation,
            conditional_mean,
            conditional_std_dev,
            standardized_residual,
            pit,
            location_log_wealth: monitor.location_eproc.log_wealth(),
            location_max_log_wealth: monitor.location_eproc.max_log_wealth(),
            dispersion_log_wealth: monitor.dispersion_eproc.log_wealth(),
            dispersion_max_log_wealth: monitor.dispersion_eproc.max_log_wealth(),
            log_threshold: monitor.location_eproc.log_threshold(),
            location_alarm: monitor.location_eproc.rejected_null(),
            dispersion_alarm: monitor.dispersion_eproc.rejected_null(),
        };
        if ![
            value.pit,
            value.location_log_wealth,
            value.location_max_log_wealth,
            value.dispersion_log_wealth,
            value.dispersion_max_log_wealth,
            value.log_threshold,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(numerical("monitor output is not representable"));
        }
        if first_alarm.is_none() && alarm {
            first_alarm = Some(Alarm {
                sequence: value.sequence,
                observation_id: value.observation.id.clone(),
                available_ns: value.observation.available_ns,
            });
        }
        let row_tag = blake3_hash(row_json(&value, &policy).as_bytes()).to_hex();
        let mut parents = vec![inputs[0].claim_id, input.claim_id];
        if let Some(previous) = rows.last() {
            parents.push(previous.claim_id);
        }
        rows.push(source_claim(
            "spatial.monitor.result/1",
            value,
            &row_tag,
            physical_support,
            parents,
            vec![policy_anchor.clone(), observation_anchor],
        ));
        inputs.push(input);
    }
    let summary = SpatialSummary {
        observations: rows.len(),
        first_alarm,
        location_alarm: monitor.location_eproc.rejected_null(),
        dispersion_alarm: monitor.dispersion_eproc.rejected_null(),
    };
    let mut frame = Encoder::new_frame(*b"SPAR", 1);
    frame.put_str(METHOD);
    frame.put_str(NO_CLAIM);
    frame.put_str(&inputs[0].claim_id.to_string());
    for row in &rows {
        frame.put_str(&row.claim_id.to_string());
    }
    frame.put_str(&summary_json(&summary));
    let digest = blake3_hash(&frame.finish()).to_hex();
    let parents = rows.iter().map(|row| row.claim_id).collect();
    let claim = source_claim(
        "spatial.monitor.assessment/1",
        summary,
        &digest,
        SpatialSupport::PeriodAggregate,
        parents,
        vec![source.clone()],
    );
    Ok(SpatialReceipt {
        source,
        policy,
        inputs,
        rows,
        claim,
        digest,
    })
}

fn js(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn policy_json(p: &Policy) -> String {
    format!(
        "{{\"model_id\":{},\"target_unit\":{},\"coordinate_frame\":{},\"coordinate_unit\":\"m\",\"support\":{},\"nugget_var\":{},\"sill_var\":{},\"range_m\":{},\"max_neighbors\":{},\"alpha\":{},\"registered_ns\":{}}}",
        js(&p.model_id),
        js(&p.target_unit),
        js(&p.coordinate_frame),
        js(&p.support),
        p.nugget_var,
        p.sill_var,
        p.range_m,
        p.max_neighbors,
        p.alpha,
        js(&p.registered_ns.to_string())
    )
}

fn observation_json(o: &Observation, policy: &Policy) -> String {
    format!(
        "{{\"id\":{},\"available_ns\":{},\"prediction_registered_ns\":{},\"x_m\":{},\"y_m\":{},\"z_m\":{},\"prior_mean\":{},\"observed_value\":{},\"target_unit\":{},\"coordinate_frame\":{},\"support\":{},\"declared_locator\":{}}}",
        js(&o.id),
        js(&o.available_ns.to_string()),
        js(&o.prediction_registered_ns.to_string()),
        o.sample.x,
        o.sample.y,
        o.sample.z,
        o.sample.prior_mean,
        o.sample.observed_value,
        js(&policy.target_unit),
        js(&policy.coordinate_frame),
        js(&policy.support),
        js(&o.declared_locator)
    )
}

fn row_json(row: &SpatialRow, policy: &Policy) -> String {
    format!(
        "{{\"sequence\":{},\"observation\":{},\"conditional_mean\":{},\"conditional_std_dev\":{},\"standardized_residual\":{},\"pit\":{},\"location_log_wealth\":{},\"location_max_log_wealth\":{},\"dispersion_log_wealth\":{},\"dispersion_max_log_wealth\":{},\"log_threshold\":{},\"location_alarm\":{},\"dispersion_alarm\":{}}}",
        row.sequence,
        observation_json(&row.observation, policy),
        row.conditional_mean,
        row.conditional_std_dev,
        row.standardized_residual,
        row.pit,
        row.location_log_wealth,
        row.location_max_log_wealth,
        row.dispersion_log_wealth,
        row.dispersion_max_log_wealth,
        row.log_threshold,
        row.location_alarm,
        row.dispersion_alarm
    )
}

fn summary_json(summary: &SpatialSummary) -> String {
    let alarm = summary.first_alarm.as_ref().map_or_else(
        || "null".into(),
        |alarm| {
            format!(
                "{{\"sequence\":{},\"observation_id\":{},\"available_ns\":{}}}",
                alarm.sequence,
                js(&alarm.observation_id),
                js(&alarm.available_ns.to_string())
            )
        },
    );
    format!(
        "{{\"observations\":{},\"first_alarm\":{alarm},\"location_alarm\":{},\"dispersion_alarm\":{},\"calibration\":\"descriptive\"}}",
        summary.observations, summary.location_alarm, summary.dispersion_alarm
    )
}

fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    let parents = claim
        .parents
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let anchors = claim
        .anchors
        .iter()
        .map(|anchor| {
            format!(
                "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&anchor.artifact_path),
                js(&Digest32(anchor.artifact_sha256).to_hex()),
                js(&anchor.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"evidence\":\"Assumed\",\"confidentiality\":\"Internal\",\"epistemic\":\"known_unknown\",\"parents\":[{parents}],\"anchors\":[{anchors}],\"value\":{value}}}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag())
    )
}

fn markdown(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\\' | '|' | '\u{60}' | '[' | ']' | '*' | '_' | '~' => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

fn claim_markdown<T>(out: &mut String, claim: &Claim<T>) {
    let parents = claim
        .parents
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("<br>");
    let locators = claim
        .anchors
        .iter()
        .map(|anchor| markdown(&anchor.locator))
        .collect::<Vec<_>>()
        .join("<br>");
    let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} |",
        claim.claim_id,
        markdown(&claim.metric_key),
        claim.value_tag(),
        parents,
        locators
    );
}

impl SpatialReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<String>] {
        &self.inputs
    }

    #[must_use]
    pub fn rows(&self) -> &[Claim<SpatialRow>] {
        &self.rows
    }

    #[must_use]
    pub fn claim(&self) -> &Claim<SpatialSummary> {
        &self.claim
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let inputs = self
            .inputs
            .iter()
            .map(|input| claim_json(input, &input.value))
            .collect::<Vec<_>>()
            .join(",");
        let rows = self
            .rows
            .iter()
            .map(|row| claim_json(row, &row_json(&row.value, &self.policy)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.spatial-assessment.v1\",\"digest\":{},\"method\":{},\"no_claim\":{},\"chronology\":\"operator-declared; no independent time proof\",\"source\":{{\"path\":{},\"sha256\":{},\"locator\":{}}},\"policy\":{},\"inputs\":[{inputs}],\"rows\":[{rows}],\"summary\":{}}}",
            js(&self.digest),
            js(METHOD),
            js(NO_CLAIM),
            js(&self.source.artifact_path),
            js(&Digest32(self.source.artifact_sha256).to_hex()),
            js(&self.source.locator),
            policy_json(&self.policy),
            claim_json(&self.claim, &summary_json(&self.claim.value))
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "# Spatial-front assessment\n\n{NO_CLAIM}\n\nMethod: {METHOD}\n\nSchema: fmine.spatial-assessment.v1. Calibration: descriptive.\n\nModel: {}. Unit: {}. Support: {}. Coordinate frame: {} (metres). These declarations apply to every observation.\n\nSource: {} (SHA-256 {}). Locator: {}. Receipt: {}.\n\nPolicy registration: {} ns (declared). Alpha: {}; nearest predecessors: {}. Variances: nugget {}, sill {}; range {} m. All registration and availability timestamps are nonnegative UTC Unix nanoseconds, declared by the operator without independent time proof.\n\nObservations: {}. Summary location alarm: {}. Summary dispersion alarm: {}.\n\n",
            markdown(&self.policy.model_id),
            markdown(&self.policy.target_unit),
            markdown(&self.policy.support),
            markdown(&self.policy.coordinate_frame),
            markdown(&self.source.artifact_path),
            Digest32(self.source.artifact_sha256).to_hex(),
            markdown(&self.source.locator),
            self.digest,
            self.policy.registered_ns,
            self.policy.alpha,
            self.policy.max_neighbors,
            self.policy.nugget_var,
            self.policy.sill_var,
            self.policy.range_m,
            self.claim.value.observations,
            self.claim.value.location_alarm,
            self.claim.value.dispersion_alarm
        );
        if let Some(alarm) = &self.claim.value.first_alarm {
            let _ = writeln!(
                out,
                "First alarm: observation {} at sequence {}, declared availability {} ns.\n",
                markdown(&alarm.observation_id),
                alarm.sequence,
                alarm.available_ns
            );
        } else {
            out.push_str(
                "No threshold crossing in the supplied observations; this is not a calibration verdict.\n\n",
            );
        }
        out.push_str(
            "## Declared observations\n\nPrior means and observed values use the declared target unit. Coordinates use the declared metre frame. Sequence preserves the supplied availability order.\n\n| Sequence | Observation | X (m) | Y (m) | Z (m) | Prior mean | Observed value |\n|---:|---|---:|---:|---:|---:|---:|\n",
        );
        for row in &self.rows {
            let value = &row.value;
            let sample = value.observation.sample;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                value.sequence,
                markdown(&value.observation.id),
                sample.x,
                sample.y,
                sample.z,
                sample.prior_mean,
                sample.observed_value
            );
        }
        out.push_str(
            "\n## Declared chronology and source locators\n\nObservation locators below are unverified declarations; the worksheet's bound source locators are recorded with the Claim bindings.\n\n| Sequence | Prediction registration (ns) | Availability (ns) | Source locator (declared) |\n|---:|---:|---:|---|\n",
        );
        for row in &self.rows {
            let value = &row.value;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                value.sequence,
                value.observation.prediction_registered_ns,
                value.observation.available_ns,
                markdown(&value.observation.declared_locator)
            );
        }
        out.push_str(
            "\n## Conditional diagnostics\n\nConditional means and standard deviations use the declared target unit. Residuals and PIT values are dimensionless.\n\n| Sequence | Conditional mean | Conditional SD | Residual z | PIT |\n|---:|---:|---:|---:|---:|\n",
        );
        for row in &self.rows {
            let value = &row.value;
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                value.sequence,
                value.conditional_mean,
                value.conditional_std_dev,
                value.standardized_residual,
                value.pit
            );
        }
        out.push_str(
            "\n## Monitor histories\n\nEach process uses alpha/2. Alarm flags retain any prior threshold crossing; current wealth can fall after its peak. Log wealth and thresholds are dimensionless.\n\n| Sequence | Monitor | Current log wealth | Peak log wealth | Log threshold | Alarm |\n|---:|---|---:|---:|---:|---|\n",
        );
        for row in &self.rows {
            let value = &row.value;
            for (monitor, current, peak, alarm) in [
                (
                    "Location",
                    value.location_log_wealth,
                    value.location_max_log_wealth,
                    value.location_alarm,
                ),
                (
                    "Dispersion",
                    value.dispersion_log_wealth,
                    value.dispersion_max_log_wealth,
                    value.dispersion_alarm,
                ),
            ] {
                let _ = writeln!(
                    out,
                    "| {} | {monitor} | {current} | {peak} | {} | {alarm} |",
                    value.sequence, value.log_threshold
                );
            }
        }
        out.push_str(
            "\n## Claim bindings\n\nEvery Claim is Assumed/Internal with known-unknown epistemic status. All anchors reference the source path and SHA-256 above; their exact worksheet locators appear below. Inputs precede diagnostic results and the assessment. Empty parents denote source-declared inputs.\n\n| Claim ID | Metric | Value tag | Parent Claim IDs | Bound worksheet locators |\n|---|---|---|---|---|\n",
        );
        for input in &self.inputs {
            claim_markdown(&mut out, input);
        }
        for row in &self.rows {
            claim_markdown(&mut out, row);
        }
        claim_markdown(&mut out, &self.claim);
        out
    }
}

#[cfg(test)]
mod tests;
