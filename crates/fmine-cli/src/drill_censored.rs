//! DESIGN (BD-168): source-bound comparison of left-censored lognormal MLE and ROS.
//! Determinism: BitwisePortable (fixed source order and existing deterministic fitting kernels).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, det, hash::sha256};
use fmine_drill::records::AssayValue;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_qaqc::censored::{
    CensoredDataset, LognormalMleFit, RosFit, fit_censored_lognormal_mle, fit_censored_ros,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::{Envelope, NextAction, journal_arguments, json_string as js, worksheet_input};

pub const VERB: &str = "drill censored";
pub const USAGE: &str = "drill censored --input <assays.toml>";
pub const NO_CLAIM: &str = "Descriptive fits of supplied assays under a declared lognormal model with noninformative left censoring. Inputs and outputs remain Assumed, Internal and uncalibrated. No DL/2 substitution or raw-assay replacement is performed. ROS modeled values are used only in its descriptive summary; modeled values above their limits are counted. The fitted lognormal has support above a physical mass fraction of one and is not a truncated physical distribution. Fitted quantiles describe model spread, not confidence intervals or calibrated uncertainty. Sampling bias, heterogeneity, dependence and omitted method errors can invalidate these fits. A successful fit does not establish QA/QC acceptance, resource or reserve classification, certificate authentication, compliance or execution authority.";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.censored-assays.v1","command":"drill censored --input <assays.toml>","input":"Quoted-string TOML, at most 1 MiB, 2..4096 observations with at least two positive measured values","top_fields":["schema","population","analyte","method","lab","unit","basis","evidence_lane","source_description","omitted_uncertainty"],"observation_fields":["sample_id","value"],"values":"Positive measured fractions in (0,1] or <DL with DL in (0,1]; no right censoring, missing values, zeros or constant substitutions","output":"Original censor tokens and source indices, separate MLE and ROS outcomes, denominators, fitted spread, log-likelihood and convergence count, high-censoring and physical-tail diagnostics; no confidence interval or acceptance score","example":"tests/drill/censored_assays.toml","side_effect":"Read-only; does not replace raw assays or persist fitted values"}"#;
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_ROWS: usize = 4096;
const FIELDS: &[&str] = &[
    "schema",
    "population",
    "analyte",
    "method",
    "lab",
    "unit",
    "basis",
    "evidence_lane",
    "source_description",
    "omitted_uncertainty",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    if words != ["drill", "censored"] || !options.is_empty() {
        return Err(USAGE.into());
    }
    Ok(Command {
        input: input.ok_or(USAGE)?.into(),
    })
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill censored declaration: {}", detail.into()),
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
            return Err(invalid(format!("{} must be quoted", field.key)));
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

fn assay(token: &str) -> Result<AssayValue, Refusal> {
    let (censored, number) = token
        .strip_prefix('<')
        .map_or((false, token), |n| (true, n));
    if number.trim() != number {
        return Err(invalid(
            "value and detection limit must not contain whitespace",
        ));
    }
    let value = number.parse::<f64>().map_err(|_| invalid("values require positive measured numbers or <DL; right censoring and placeholders are unsupported"))?;
    if !(value.is_finite() && value > 0.0 && value <= 1.0) {
        return Err(invalid(
            "measured values and detection limits require positive mass fractions in (0,1]; zero and underflow refuse",
        ));
    }
    Ok(if censored {
        AssayValue::BelowDetection { limit: value }
    } else {
        AssayValue::Measured {
            mass_fraction: value,
        }
    })
}

fn checked_mle(dataset: &CensoredDataset) -> Result<LognormalMleFit, Refusal> {
    let fit = fit_censored_lognormal_mle(dataset).map_err(|e| e.to_refusal())?;
    if [
        fit.mu_log,
        fit.sigma_log,
        fit.log_likelihood,
        fit.median,
        fit.arithmetic_mean,
        fit.arithmetic_sd,
        fit.p90,
    ]
    .iter()
    .any(|x| !x.is_finite())
        || [
            fit.sigma_log,
            fit.median,
            fit.arithmetic_mean,
            fit.arithmetic_sd,
            fit.p90,
        ]
        .iter()
        .any(|x| *x <= 0.0)
    {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "MLE lost a positive fitted moment or produced nonfinite diagnostics",
        ));
    }
    Ok(fit)
}

fn checked_ros(dataset: &CensoredDataset) -> Result<RosFit, Refusal> {
    let fit = fit_censored_ros(dataset).map_err(|e| e.to_refusal())?;
    if [
        fit.mu_log,
        fit.sigma_log,
        fit.r_squared,
        fit.mean,
        fit.sd,
        fit.p25,
        fit.median,
        fit.p75,
        fit.p90,
    ]
    .iter()
    .any(|x| !x.is_finite())
        || [
            fit.sigma_log,
            fit.mean,
            fit.sd,
            fit.p25,
            fit.median,
            fit.p75,
            fit.p90,
        ]
        .iter()
        .any(|x| *x <= 0.0)
    {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "ROS lost a positive fitted moment or produced nonfinite diagnostics",
        ));
    }
    Ok(fit)
}

fn failure(error: &Refusal) -> String {
    format!(
        "{{\"status\":\"refused\",\"refusal\":{{\"code\":{},\"what\":{},\"fix\":{}}},\"fit\":null}}",
        js(error.code),
        js(&error.what),
        js(error.fix)
    )
}

fn render_mle(result: &Result<LognormalMleFit, Refusal>, markdown: &mut String) -> String {
    match result {
        Err(error) => {
            let _ = writeln!(
                markdown,
                "## Lognormal MLE\n\n**Refused:** {} — {}\n",
                error.code,
                worksheet_input::markdown_cell(&error.what)
            );
            failure(error)
        }
        Ok(fit) => {
            let tail = det::norm_cdf(fit.mu_log / fit.sigma_log);
            let _ = writeln!(
                markdown,
                "## Lognormal MLE\n\nLog mean {}; log SD {}. Fitted mean {}; SD {}; median {}; P90 {}. Log-likelihood {}; iterations {}. Fitted probability above mass fraction one: {}. High censoring: {}.\n",
                fit.mu_log,
                fit.sigma_log,
                fit.arithmetic_mean,
                fit.arithmetic_sd,
                fit.median,
                fit.p90,
                fit.log_likelihood,
                fit.iterations,
                tail,
                fit.high_censoring_warning
            );
            format!(
                "{{\"status\":\"ok\",\"fit\":{{\"mu_log\":{},\"sigma_log\":{},\"mean\":{},\"sd\":{},\"median\":{},\"p90\":{},\"log_likelihood\":{},\"iterations\":{},\"fitted_lognormal_probability_above_one\":{},\"high_censoring_warning\":{},\"confidence_interval\":null}}}}",
                fit.mu_log,
                fit.sigma_log,
                fit.arithmetic_mean,
                fit.arithmetic_sd,
                fit.median,
                fit.p90,
                fit.log_likelihood,
                fit.iterations,
                tail,
                fit.high_censoring_warning
            )
        }
    }
}

fn render_ros(result: &Result<RosFit, Refusal>, markdown: &mut String) -> String {
    match result {
        Err(error) => {
            let _ = writeln!(
                markdown,
                "## Regression on Order Statistics\n\n**Refused:** {} — {}\n",
                error.code,
                worksheet_input::markdown_cell(&error.what)
            );
            failure(error)
        }
        Ok(fit) => {
            let tail = det::norm_cdf(fit.mu_log / fit.sigma_log);
            let _ = writeln!(
                markdown,
                "## Regression on Order Statistics\n\nLog intercept {}; log slope {}; R² {}. Descriptive mean {}; sample SD {}; P25 {}; median {}; P75 {}; P90 {}. Modeled censored values above their own detection limit: {}. Fitted lognormal probability above mass fraction one: {}. High censoring: {}.\n",
                fit.mu_log,
                fit.sigma_log,
                fit.r_squared,
                fit.mean,
                fit.sd,
                fit.p25,
                fit.median,
                fit.p75,
                fit.p90,
                fit.modeled_above_dl_count,
                tail,
                fit.high_censoring_warning
            );
            format!(
                "{{\"status\":\"ok\",\"fit\":{{\"mu_log\":{},\"sigma_log\":{},\"r_squared\":{},\"mean\":{},\"sd\":{},\"p25\":{},\"median\":{},\"p75\":{},\"p90\":{},\"modeled_above_dl_count\":{},\"fitted_lognormal_probability_above_one\":{},\"high_censoring_warning\":{},\"confidence_interval\":null}}}}",
                fit.mu_log,
                fit.sigma_log,
                fit.r_squared,
                fit.mean,
                fit.sd,
                fit.p25,
                fit.median,
                fit.p75,
                fit.p90,
                fit.modeled_above_dl_count,
                tail,
                fit.high_censoring_warning
            )
        }
    }
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(&command.input), "censored assays", MAX_BYTES)?;
        let doc = parse_subset(&text).map_err(|e| invalid(e.to_string()))?;
        let top = fields(&doc.top, FIELDS)?;
        if top["schema"] != "fmine.censored-assays.v1"
            || top["unit"] != "mass_fraction"
            || !matches!(top["basis"], "dry" | "wet")
        {
            return Err(invalid(
                "declare schema fmine.censored-assays.v1, unit mass_fraction and basis dry or wet; no implicit conversion",
            ));
        }
        if !matches!(top["evidence_lane"], "synthetic" | "internal") {
            return Err(invalid("evidence_lane must be synthetic or internal"));
        }
        if !(2..=MAX_ROWS).contains(&doc.rows.len()) {
            return Err(invalid("provide 2..4096 observations"));
        }
        let mut ids = BTreeSet::new();
        let mut values = Vec::with_capacity(doc.rows.len());
        let mut observations = Vec::with_capacity(doc.rows.len());
        let mut markdown = format!(
            "# Left-censored assay comparison\n\nPopulation **{}** · analyte **{}** · method **{}** · lab **{}** · mass fraction, {} basis. Evidence lane: {}.\n\n| Index | Sample | Original token | Source line |\n|---:|---|---|---:|\n",
            worksheet_input::markdown_cell(top["population"]),
            worksheet_input::markdown_cell(top["analyte"]),
            worksheet_input::markdown_cell(top["method"]),
            worksheet_input::markdown_cell(top["lab"]),
            top["basis"],
            top["evidence_lane"]
        );
        let mut n_censored = 0;
        for (index, row) in doc.rows.iter().enumerate() {
            if row.table != "observation" {
                return Err(invalid(format!(
                    "unknown table {} at line {}",
                    row.table, row.line
                )));
            }
            let f = fields(&row.fields, &["sample_id", "value"])?;
            if f["sample_id"].len() > 256 || !ids.insert(f["sample_id"].to_owned()) {
                return Err(invalid("sample IDs must be unique and at most 256 bytes"));
            }
            let value = assay(f["value"])?;
            let value_json = match value {
                AssayValue::Measured { mass_fraction } => {
                    format!("\"kind\":\"measured\",\"value\":{mass_fraction}")
                }
                AssayValue::BelowDetection { limit } => {
                    n_censored += 1;
                    format!("\"kind\":\"below_detection\",\"limit\":{limit},\"substitution\":null")
                }
                AssayValue::AboveUpper { .. } => {
                    return Err(invalid("right censoring is unsupported"));
                }
            };
            observations.push(format!("{{\"index\":{index},\"sample_id\":{},\"source_line\":{},\"token\":{},{value_json}}}", js(f["sample_id"]), row.line, js(f["value"])));
            let _ = writeln!(
                markdown,
                "| {index} | {} | {} | {} |",
                worksheet_input::markdown_cell(f["sample_id"]),
                worksheet_input::markdown_cell(f["value"]),
                row.line
            );
            values.push(value);
        }
        let n = values.len();
        let n_uncensored = n - n_censored;
        if n_uncensored < 2 {
            return Err(invalid(
                "both fits require at least two positive uncensored observations",
            ));
        }
        let dataset = CensoredDataset { values };
        let mle = checked_mle(&dataset);
        let ros = checked_ros(&dataset);
        if let (Err(m), Err(r)) = (&mle, &ros) {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "both methods refused: MLE [{}] {}; ROS [{}] {}",
                    m.code, m.what, r.code, r.what
                ),
            ));
        }
        let state = if mle.is_ok() && ros.is_ok() {
            "complete"
        } else {
            "partial"
        };
        let _ = writeln!(
            markdown,
            "\nComparison: **{state}**. {n} observations: {n_uncensored} measured, {n_censored} left-censored. Each method uses the complete declared dataset.\n"
        );
        let mle_json = render_mle(&mle, &mut markdown);
        let ros_json = render_ros(&ros, &mut markdown);
        let declarations = FIELDS
            .iter()
            .filter(|key| **key != "schema")
            .map(|key| format!("{}:{}", js(key), js(top[key])))
            .collect::<Vec<_>>()
            .join(",");
        let report = format!(
            "{{\"schema\":\"fmine.censored-assay-report.v1\",{declarations},\"source\":{},\"comparison_state\":{},\"model\":\"positive_untruncated_lognormal_noninformative_left_censoring\",\"n\":{n},\"n_uncensored\":{n_uncensored},\"n_censored\":{n_censored},\"raw_assays_replaced\":false,\"observations\":[{}],\"mle\":{mle_json},\"ros\":{ros_json}}}",
            worksheet_input::source_json(&source, text.len()),
            js(state),
            observations.join(",")
        );
        let claim = Claim::new(
            "drill.censored-assays/1",
            (mle, ros),
            &report,
            Representation::Placeholder {
                token: "descriptive_fits_no_estimation_confidence_interval".into(),
            },
            SpatialSupport::PointCore,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![source.clone()],
        );
        claim.check_id()?;
        let receipt = sha256(report.as_bytes());
        let _ = write!(
            markdown,
            "\nSource description: {}\n\nOmitted uncertainty: {}\n\nSource: `{}`; SHA-256: `{}`.\n\nClaim: `{}`. Receipt SHA-256: `{receipt}`.\n\n{NO_CLAIM}\n",
            worksheet_input::markdown_cell(top["source_description"]),
            worksheet_input::markdown_cell(top["omitted_uncertainty"]),
            worksheet_input::markdown_cell(&source.artifact_path),
            Digest32(source.artifact_sha256),
            claim.claim_id
        );
        let mut envelope = Envelope::ok(
            VERB,
            format!(
                "{{\"receipt_sha256\":\"{receipt}\",\"claim\":{{\"id\":{},\"metric\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"calibration\":\"uncalibrated\",\"confidentiality\":\"Internal\",\"support\":\"point_assay_samples\",\"additivity\":\"NonAdditive\"}},\"report\":{report}}}",
                js(&claim.claim_id.to_string()),
                js(&claim.metric_key)
            ),
            vec![],
        );
        if state == "partial" {
            envelope.warnings.push("Only one fitting method succeeded; the other method's typed refusal remains in the report. This is a partial comparison.".into());
        }
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            VERB,
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the censored assay input and model contract",
            )],
        )
    });
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
