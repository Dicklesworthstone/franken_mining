//! DESIGN (BD-169, BD-283): source-bound replay of the existing sequential CRM monitor.
//! Determinism: DeterministicPerIsa (inherits the e-process kernel; preserves declared stream order).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_drill::records::AssayMethod;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_qaqc::crm::SdBasis;
use fmine_qaqc::monitors::{
    AlarmScope, CrmMonitorConfig, CrmMonitorSnapshot, CrmSequentialMonitor, CrmStreamKey,
    CrmStreamValue, MonitorAlarm, MonitorKind,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::{Envelope, NextAction, journal_arguments, json_string as js, worksheet_input};

pub const VERB: &str = "drill monitor";
pub const USAGE: &str = "drill monitor --input <stream.toml>";
pub const NO_CLAIM: &str = "Conditional replay of a declared CRM stream, fixed reference and supplied policy. All inputs and outputs remain Assumed/Internal and uncalibrated. Only the e-process has an anytime-valid threshold, conditional on the specified independent Gaussian null and a policy fixed before observations. Repeated pattern, sign and dispersion checks have no combined or anytime false-alarm guarantee. Log wealth is evidence against the declared null, not a probability of tampering or model correctness. Certificate authenticity, policy preregistration, observation chronology and completeness are not verified. Alarms propose review; no InvestigationCase, assay release, custody, compliance or execution authority is written.";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.crm-monitor.v1","command":"drill monitor --input <stream.toml>","input":"Quoted-string TOML, at most 1 MiB, with 1..4096 ordered observations and unique sample IDs", "top_fields":["schema","crm_id","lab","analyte","method","unit","basis","certificate_source","policy_id","policy_version","policy_source","evidence_lane","omitted_uncertainty","certified_mean","reference_sd","sd_basis","eproc_alpha","sign_min_samples","sign_alpha","dispersion_window","collapse_ratio_threshold","dispersion_alpha"],"observation_fields":["sample_id","value"],"units":"Explicit mass_fraction in [0,1], on a common dry or wet basis; no conversion", "sd_basis":"sigma_c or sigma_l; fixed external reference, never fitted from this stream","order":"Declared table order, replayed from an empty monitor; every prefix retains its source line and index","output":"Per-prefix log wealth, final state and alarms with scope and originating sample; all authority flags false","example":"tests/drill/crm_monitor.toml","side_effect":"Read-only; does not persist monitor state or write investigation records"}"#;
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_OBSERVATIONS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    if words != ["drill", "monitor"] || !options.is_empty() {
        return Err(USAGE.into());
    }
    Ok(Command {
        input: input.ok_or(USAGE)?.into(),
    })
}

struct Observation {
    sample_id: String,
    source_line: usize,
    value: f64,
}

struct Declaration {
    config: CrmMonitorConfig,
    analyte: String,
    basis: String,
    certificate_source: String,
    policy_id: String,
    policy_version: String,
    policy_source: String,
    lane: String,
    omitted_uncertainty: String,
    observations: Vec<Observation>,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill monitor declaration: {}", detail.into()),
    )
}

fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
) -> Result<BTreeMap<&'a str, &'a str>, Refusal> {
    let mut result = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) {
            return Err(invalid(format!(
                "unknown field {} at line {}",
                field.key, field.line
            )));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(invalid(format!("{} must be a quoted string", field.key)));
        };
        if value.is_empty()
            || value.trim() != value
            || value.len() > 4096
            || value.chars().any(char::is_control)
        {
            return Err(invalid(format!(
                "{} must be bounded nonempty text without controls or surrounding whitespace",
                field.key
            )));
        }
        result.insert(field.key.as_str(), value.as_str());
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(invalid(format!("missing {key}")));
        }
    }
    Ok(result)
}

fn number(fields: &BTreeMap<&str, &str>, key: &str) -> Result<f64, Refusal> {
    let token = fields[key];
    let value = token
        .parse::<f64>()
        .map_err(|_| invalid(format!("{key} must be an explicit finite number")))?;
    let nonzero = token
        .split(['e', 'E'])
        .next()
        .is_some_and(|s| s.bytes().any(|b| matches!(b, b'1'..=b'9')));
    if !value.is_finite() || (value == 0.0 && nonzero) {
        return Err(invalid(format!(
            "{key} is nonfinite or loses a nonzero value to underflow"
        )));
    }
    Ok(value)
}

fn count(fields: &BTreeMap<&str, &str>, key: &str) -> Result<usize, Refusal> {
    let token = fields[key];
    let n = token
        .parse::<usize>()
        .map_err(|_| invalid(format!("{key} must be an integer")))?;
    if !(6..=MAX_OBSERVATIONS).contains(&n) || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(format!("{key} must lie in 6..={MAX_OBSERVATIONS}")));
    }
    Ok(n)
}

fn declaration(text: &str) -> Result<Declaration, Refusal> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    let top = fields(
        &doc.top,
        &[
            "schema",
            "crm_id",
            "lab",
            "analyte",
            "method",
            "unit",
            "basis",
            "certificate_source",
            "policy_id",
            "policy_version",
            "policy_source",
            "evidence_lane",
            "omitted_uncertainty",
            "certified_mean",
            "reference_sd",
            "sd_basis",
            "eproc_alpha",
            "sign_min_samples",
            "sign_alpha",
            "dispersion_window",
            "collapse_ratio_threshold",
            "dispersion_alpha",
        ],
    )?;
    if top["schema"] != "fmine.crm-monitor.v1" || top["unit"] != "mass_fraction" {
        return Err(invalid(
            "schema must be fmine.crm-monitor.v1 and unit must be mass_fraction",
        ));
    }
    if !matches!(top["basis"], "dry" | "wet")
        || !matches!(top["evidence_lane"], "synthetic" | "internal")
    {
        return Err(invalid(
            "basis must be dry or wet; evidence_lane must be synthetic or internal",
        ));
    }
    let method = AssayMethod::ALL
        .iter()
        .copied()
        .find(|m| m.code() == top["method"])
        .ok_or_else(|| invalid("method must use a registered assay method code, e.g. fa_aas"))?;
    let sd_basis = match top["sd_basis"] {
        "sigma_c" => SdBasis::CertifiedWithinLabSigmaC,
        "sigma_l" => SdBasis::CertifiedBetweenLabSigmaL,
        _ => {
            return Err(invalid(
                "sd_basis must be sigma_c or sigma_l, fixed independently of this stream",
            ));
        }
    };
    let certified_mean = number(&top, "certified_mean")?;
    if !(0.0..=1.0).contains(&certified_mean) {
        return Err(invalid("certified_mean must lie in [0,1]"));
    }
    let config = CrmMonitorConfig {
        key: CrmStreamKey {
            crm_id: top["crm_id"].into(),
            lab: top["lab"].into(),
            method,
            sd_basis,
        },
        certified_mean,
        reference_sd: number(&top, "reference_sd")?,
        eproc_alpha: number(&top, "eproc_alpha")?,
        sign_min_samples: count(&top, "sign_min_samples")?,
        sign_alpha: number(&top, "sign_alpha")?,
        dispersion_window: count(&top, "dispersion_window")?,
        collapse_ratio_threshold: number(&top, "collapse_ratio_threshold")?,
        dispersion_alpha: number(&top, "dispersion_alpha")?,
    };
    config.validate().map_err(|e| e.to_refusal())?;
    if !(1..=MAX_OBSERVATIONS).contains(&doc.rows.len()) {
        return Err(invalid("provide 1..4096 observations"));
    }
    let mut ids = BTreeSet::new();
    let mut observations = Vec::with_capacity(doc.rows.len());
    for row in &doc.rows {
        if row.table != "observation" {
            return Err(invalid(format!(
                "unknown table {} at line {}",
                row.table, row.line
            )));
        }
        let f = fields(&row.fields, &["sample_id", "value"])?;
        let value = number(&f, "value")?;
        if !(0.0..=1.0).contains(&value) {
            return Err(invalid(
                "every observation must be a measured mass fraction in [0,1]; censored results cannot be substituted",
            ));
        }
        if f["sample_id"].len() > 256 || !ids.insert(f["sample_id"].to_owned()) {
            return Err(invalid(
                "sample_id values must be unique and at most 256 bytes",
            ));
        }
        observations.push(Observation {
            sample_id: f["sample_id"].into(),
            source_line: row.line,
            value,
        });
    }
    Ok(Declaration {
        config,
        analyte: top["analyte"].into(),
        basis: top["basis"].into(),
        certificate_source: top["certificate_source"].into(),
        policy_id: top["policy_id"].into(),
        policy_version: top["policy_version"].into(),
        policy_source: top["policy_source"].into(),
        lane: top["evidence_lane"].into(),
        omitted_uncertainty: top["omitted_uncertainty"].into(),
        observations,
    })
}

fn kind(monitor: MonitorKind) -> &'static str {
    match monitor {
        MonitorKind::EProcessDrift => "eprocess_drift",
        MonitorKind::TwoConsecutiveBeyond2Sd => "two_consecutive_beyond_2sd",
        MonitorKind::FourConsecutiveBeyond1Sd => "four_consecutive_beyond_1sd",
        MonitorKind::SignImbalance => "sign_imbalance",
        MonitorKind::DispersionCollapse => "dispersion_collapse",
    }
}

fn scope(value: AlarmScope) -> &'static str {
    match value {
        AlarmScope::CalibrationOrBias => "calibration_or_bias",
        AlarmScope::Integrity => "integrity",
    }
}

fn option(value: Option<usize>) -> String {
    value.map_or_else(|| "null".into(), |v| v.to_string())
}
fn optional_number(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), |v| v.to_string())
}

fn alarm_json(alarm: &MonitorAlarm, input: &Declaration) -> String {
    let source = &input.observations[alarm.step - 1];
    format!(
        "{{\"monitor\":{},\"scope\":{},\"step\":{},\"index\":{},\"sample_id\":{},\"source_line\":{},\"statistic\":{},\"detail\":{}}}",
        js(kind(alarm.monitor)),
        js(scope(alarm.scope)),
        alarm.step,
        alarm.step - 1,
        js(&source.sample_id),
        source.source_line,
        alarm.statistic,
        js(&alarm.detail)
    )
}

fn replay(
    input: &Declaration,
    source: &SourceAnchor,
    bytes: usize,
) -> Result<(String, String, CrmMonitorSnapshot), Refusal> {
    let c = &input.config;
    let mut monitor = CrmSequentialMonitor::new(c.clone()).map_err(|e| e.to_refusal())?;
    let mut prefixes = Vec::with_capacity(input.observations.len());
    let mut markdown = format!(
        "# CRM sequential monitor\n\nCRM **{}** · lab **{}** · analyte **{}** · method **{}** · mass fraction, {} basis. Evidence lane: {}.\n\nFixed reference: mean {}, SD {} on {}. Declared policy: {} version {}.\n\n| Step | Sample | Value | Log wealth | Maximum log wealth | New alarms | Source line |\n|---:|---|---:|---:|---:|---|---:|\n",
        worksheet_input::markdown_cell(&c.key.crm_id),
        worksheet_input::markdown_cell(&c.key.lab),
        worksheet_input::markdown_cell(&input.analyte),
        c.key.method.code(),
        input.basis,
        input.lane,
        c.certified_mean,
        c.reference_sd,
        c.key.sd_basis.code(),
        worksheet_input::markdown_cell(&input.policy_id),
        worksheet_input::markdown_cell(&input.policy_version)
    );
    for (index, observation) in input.observations.iter().enumerate() {
        let alarms = monitor
            .observe(CrmStreamValue {
                measured: observation.value,
            })
            .map_err(|e| e.to_refusal())?;
        let snapshot = monitor.snapshot();
        if [
            snapshot.eproc_log_wealth,
            snapshot.eproc_max_log_wealth,
            snapshot.eproc_log_threshold,
        ]
        .iter()
        .any(|x| !x.is_finite())
            || snapshot
                .latest_dispersion_ratio
                .is_some_and(|x| !x.is_finite())
            || alarms.iter().any(|x| !x.statistic.is_finite())
        {
            return Err(invalid("monitor produced nonfinite diagnostics"));
        }
        let alarm_jsons = alarms
            .iter()
            .map(|a| alarm_json(a, input))
            .collect::<Vec<_>>()
            .join(",");
        prefixes.push(format!("{{\"index\":{index},\"step\":{},\"sample_id\":{},\"source_line\":{},\"value\":{},\"eproc_log_wealth\":{},\"eproc_max_log_wealth\":{},\"latest_dispersion_ratio\":{},\"new_alarms\":[{alarm_jsons}]}}",
            index + 1, js(&observation.sample_id), observation.source_line, observation.value, snapshot.eproc_log_wealth, snapshot.eproc_max_log_wealth, optional_number(snapshot.latest_dispersion_ratio)));
        let names = alarms
            .iter()
            .map(|a| kind(a.monitor))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} | {} |",
            index + 1,
            worksheet_input::markdown_cell(&observation.sample_id),
            observation.value,
            snapshot.eproc_log_wealth,
            snapshot.eproc_max_log_wealth,
            worksheet_input::markdown_cell(&names),
            observation.source_line
        );
    }
    let snapshot = monitor.snapshot();
    let alarms = snapshot
        .alarms
        .iter()
        .map(|a| alarm_json(a, input))
        .collect::<Vec<_>>()
        .join(",");
    let _ = write!(
        markdown,
        "\n{} observations; {} alarms. E-process log threshold: {}. First e-process alarm: {}. First dispersion-collapse alarm: {}.\n\n| Step | Monitor | Scope | Statistic | Reason |\n|---:|---|---|---:|---|\n",
        snapshot.n_steps,
        snapshot.alarms.len(),
        snapshot.eproc_log_threshold,
        option(snapshot.eproc_alarm_step),
        option(snapshot.dispersion_collapse_step)
    );
    for alarm in &snapshot.alarms {
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} |",
            alarm.step,
            worksheet_input::markdown_cell(kind(alarm.monitor)),
            scope(alarm.scope),
            alarm.statistic,
            worksheet_input::markdown_cell(&alarm.detail)
        );
    }
    let report = format!(
        "{{\"schema\":\"fmine.crm-monitor-report.v1\",\"crm_id\":{},\"lab\":{},\"analyte\":{},\"method\":{},\"unit\":\"mass_fraction\",\"basis\":{},\"evidence_lane\":{},\"source\":{},\"certificate\":{{\"source\":{},\"verification\":\"not_performed\"}},\"policy\":{{\"id\":{},\"version\":{},\"source\":{},\"preregistration\":\"not_verified\",\"eproc_alpha\":{},\"sign_min_samples\":{},\"sign_alpha\":{},\"dispersion_window\":{},\"collapse_ratio_threshold\":{},\"dispersion_alpha\":{}}},\"reference\":{{\"certified_mean\":{},\"reference_sd\":{},\"sd_basis\":{}}},\"order\":\"declared_observation_table_order\",\"null\":\"independent_normal_with_fixed_mean_and_sd\",\"anytime_valid_component\":\"eprocess_only_conditional_on_declared_null\",\"omitted_uncertainty\":{},\"n\":{},\"eproc_log_wealth\":{},\"eproc_max_log_wealth\":{},\"eproc_log_threshold\":{},\"eproc_alarm_step\":{},\"dispersion_collapse_step\":{},\"latest_dispersion_ratio\":{},\"prefixes\":[{}],\"alarms\":[{alarms}]}}",
        js(&c.key.crm_id),
        js(&c.key.lab),
        js(&input.analyte),
        js(c.key.method.code()),
        js(&input.basis),
        js(&input.lane),
        worksheet_input::source_json(source, bytes),
        js(&input.certificate_source),
        js(&input.policy_id),
        js(&input.policy_version),
        js(&input.policy_source),
        c.eproc_alpha,
        c.sign_min_samples,
        c.sign_alpha,
        c.dispersion_window,
        c.collapse_ratio_threshold,
        c.dispersion_alpha,
        c.certified_mean,
        c.reference_sd,
        js(c.key.sd_basis.code()),
        js(&input.omitted_uncertainty),
        snapshot.n_steps,
        snapshot.eproc_log_wealth,
        snapshot.eproc_max_log_wealth,
        snapshot.eproc_log_threshold,
        option(snapshot.eproc_alarm_step),
        option(snapshot.dispersion_collapse_step),
        optional_number(snapshot.latest_dispersion_ratio),
        prefixes.join(",")
    );
    let _ = write!(
        markdown,
        "\nCertificate source: {}. Verification: **not performed**.\n\nPolicy source: {}. Preregistration: **not verified**.\n\nOmitted uncertainty: {}\n\nSource: `{}`; SHA-256: `{}`.\n\n{NO_CLAIM}\n",
        worksheet_input::markdown_cell(&input.certificate_source),
        worksheet_input::markdown_cell(&input.policy_source),
        worksheet_input::markdown_cell(&input.omitted_uncertainty),
        worksheet_input::markdown_cell(&source.artifact_path),
        Digest32(source.artifact_sha256)
    );
    Ok((report, markdown, snapshot))
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(&command.input), "CRM stream", MAX_BYTES)?;
        let input = declaration(&text)?;
        let (report, mut markdown, result) = replay(&input, &source, text.len())?;
        let claim = Claim::new(
            "drill.crm-monitor/1",
            result,
            &report,
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![source],
        );
        claim.check_id()?;
        let receipt = sha256(report.as_bytes());
        let _ = writeln!(
            markdown,
            "\nClaim: `{}`. Receipt SHA-256: `{receipt}`.",
            claim.claim_id
        );
        let mut envelope = Envelope::ok(
            VERB,
            format!(
                "{{\"receipt_sha256\":\"{receipt}\",\"claim\":{{\"id\":{},\"metric\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"calibration\":\"uncalibrated\",\"confidentiality\":\"Internal\",\"support\":\"point_core_crm_samples\",\"additivity\":\"NonAdditive\"}},\"report\":{report}}}",
                js(&claim.claim_id.to_string()),
                js(&claim.metric_key)
            ),
            vec![],
        );
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            VERB,
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the CRM stream and policy contract",
            )],
        )
    });
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
