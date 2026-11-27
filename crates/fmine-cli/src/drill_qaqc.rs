//! DESIGN (BD-167, plan §12.4): read-only, source-bound CRM batch scoring under explicit SD bases.
//! Determinism: BitwisePortable (ordered input, deterministic arithmetic and source hashes).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_drill::qaqc::{
    CrmAssessment, CrmBatch, CrmPolicy, CrmState, MAX_CRM_OBSERVATIONS, score_crm_batch,
};
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::{Envelope, NextAction, journal_arguments, json_string as js, worksheet_input};

pub const VERB: &str = "drill qaqc";
pub const USAGE: &str = "drill qaqc --input <batch.toml>";
pub const NO_CLAIM: &str = "CRM screening of supplied observations and declared policy only. Inputs are Assumed, Internal and uncalibrated. The certificate source is a declaration; its contents, signature and applicability are not verified. The signed bias interval assumes independent normal replicates and a fixed certified mean; omitted reference uncertainty and shared errors can change coverage. Threshold results and CANMET do not establish whole-batch QA/QC acceptance, an anytime-valid alarm, assay release, custody, compliance or execution authority.";
pub const ROBOT_DOCS_JSON: &str = r#"{
"schema":"fmine.crm-batch.v1",
"command":"drill qaqc --input <batch.toml>",
"input":"Quoted-string TOML, at most 1 MiB, with 2..4096 ordered observations and unique sample IDs",
"top_fields":["schema","crm_id","lab","analyte","method","unit","certificate_source","evidence_lane","omitted_uncertainty","certified_mean","sigma_c","sigma_l","lab_running_sw","warning_k","failure_k","confidence"],
"observation_fields":["sample_id","value"],
"units":"All values, the certified mean and all SDs use one declared analyte, method and unit; no conversions or method merging",
"sd_bases":"sigma_c = certified within-lab; sigma_l = certified between-lab; lab_running_sw = explicitly supplied lab running SD, never substituted with this batch's sample SD",
"policy":"All three SD bases are scored. Strict binary64 exceedance triggers warning/failure; no decimal rounding tolerance is inferred. Equality of computed values passes the corresponding threshold. Warning counts include failures; individual states are exclusive",
"canmet":"Absolute mean bias <= 2*sqrt(sigma_l^2 + lab_running_sw^2/n)",
"bias_interval":"Signed mean-minus-reference Student t interval using batch sample SD and n-1 degrees of freedom; conditional on independent normal observations and a fixed certified mean",
"evidence_lane":"synthetic or internal; source SHA-256 and sample line locators bind the declared batch; certificate verification is not performed",
"output":"Typed Claim, every observation under every SD basis, denominators, CANMET, signed bias interval, source identity and omitted uncertainty; all authority flags false",
"example":"tests/drill/crm_batch.toml",
"side_effect":"Read-only; no assay, custody, QA/QC acceptance or authority records are written"
}"#;
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    if words != ["drill", "qaqc"] || !options.is_empty() {
        return Err(USAGE.into());
    }
    Ok(Command {
        input: input.ok_or(USAGE)?.into(),
    })
}

struct Declaration {
    crm_id: String,
    lab: String,
    analyte: String,
    method: String,
    unit: String,
    certificate_source: String,
    lane: String,
    omitted_uncertainty: String,
    observations: Vec<(String, usize)>,
    batch: CrmBatch,
    policy: CrmPolicy,
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill qaqc declaration: {}", detail.into()),
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
                "{} must be nonempty bounded text without surrounding whitespace or controls",
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

fn number(values: &BTreeMap<&str, &str>, key: &str) -> Result<f64, Refusal> {
    values.get(key).and_then(|token| {
        let value = token.parse::<f64>().ok()?;
        // A finite parse can still silently underflow a nonzero declared concentration or SD.
        let nonzero_mantissa = token.split(['e', 'E']).next()?.bytes()
            .any(|byte| matches!(byte, b'1'..=b'9'));
        (value.is_finite() && (value != 0.0 || !nonzero_mantissa)).then_some(value)
    }).ok_or_else(|| invalid(format!("{key} must be an explicit finite, non-underflowing number; censored or missing values cannot be scored as measurements")))
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
            "certificate_source",
            "evidence_lane",
            "omitted_uncertainty",
            "certified_mean",
            "sigma_c",
            "sigma_l",
            "lab_running_sw",
            "warning_k",
            "failure_k",
            "confidence",
        ],
    )?;
    if top["schema"] != "fmine.crm-batch.v1" {
        return Err(invalid("schema must be fmine.crm-batch.v1"));
    }
    if !matches!(top["evidence_lane"], "synthetic" | "internal") {
        return Err(invalid("evidence_lane must be synthetic or internal"));
    }
    if !(2..=MAX_CRM_OBSERVATIONS).contains(&doc.rows.len()) {
        return Err(invalid(format!(
            "provide 2..={MAX_CRM_OBSERVATIONS} observations"
        )));
    }
    let mut ids = BTreeSet::new();
    let mut observations = Vec::with_capacity(doc.rows.len());
    let mut values = Vec::with_capacity(doc.rows.len());
    for row in &doc.rows {
        if row.table != "observation" {
            return Err(invalid(format!(
                "unknown table {} at line {}",
                row.table, row.line
            )));
        }
        let fields = fields(&row.fields, &["sample_id", "value"])?;
        let id = fields["sample_id"];
        if id.len() > 256 || !ids.insert(id.to_owned()) {
            return Err(invalid(
                "sample_id values must be unique and at most 256 bytes",
            ));
        }
        observations.push((id.to_owned(), row.line));
        values.push(number(&fields, "value")?);
    }
    Ok(Declaration {
        crm_id: top["crm_id"].into(),
        lab: top["lab"].into(),
        analyte: top["analyte"].into(),
        method: top["method"].into(),
        unit: top["unit"].into(),
        certificate_source: top["certificate_source"].into(),
        lane: top["evidence_lane"].into(),
        omitted_uncertainty: top["omitted_uncertainty"].into(),
        observations,
        batch: CrmBatch {
            certified_mean: number(&top, "certified_mean")?,
            sigma_c: number(&top, "sigma_c")?,
            sigma_l: number(&top, "sigma_l")?,
            lab_running_sw: number(&top, "lab_running_sw")?,
            values,
        },
        policy: CrmPolicy {
            warning_k: number(&top, "warning_k")?,
            failure_k: number(&top, "failure_k")?,
            confidence: number(&top, "confidence")?,
        },
    })
}

fn state(value: &CrmState) -> &'static str {
    match value {
        CrmState::Pass => "pass",
        CrmState::Warning => "warning",
        CrmState::Failure => "failure",
    }
}

fn render(
    input: &Declaration,
    result: &CrmAssessment,
    source: &SourceAnchor,
    bytes: usize,
) -> (String, String) {
    let mut bases = Vec::new();
    let mut markdown = format!(
        "# CRM batch assessment\n\nCRM **{}** · lab **{}** · analyte **{}** · method **{}** · unit **{}**. Evidence lane: {}.\n\n{} observations; certified mean {}; batch mean {}; sample SD {}; standard error {}.\n\n| SD basis | SD | Warning limit | Failure limit | Warnings, including failures | Failures | N |\n|---|---:|---:|---:|---:|---:|---:|\n",
        worksheet_input::markdown_cell(&input.crm_id),
        worksheet_input::markdown_cell(&input.lab),
        worksheet_input::markdown_cell(&input.analyte),
        worksheet_input::markdown_cell(&input.method),
        worksheet_input::markdown_cell(&input.unit),
        input.lane,
        result.n,
        input.batch.certified_mean,
        result.mean,
        result.sample_sd,
        result.standard_error,
    );
    for basis in &result.single_value {
        let rows = basis.results.iter().map(|check| {
            let (id, line) = &input.observations[check.index];
            format!("{{\"index\":{},\"sample_id\":{},\"source_line\":{line},\"value\":{},\"deviation\":{},\"state\":{}}}",
                check.index, js(id), input.batch.values[check.index], check.deviation, js(state(&check.state)))
        }).collect::<Vec<_>>().join(",");
        bases.push(format!("{{\"basis\":{},\"sd\":{},\"warning_limit\":{},\"failure_limit\":{},\"warnings\":{},\"failures\":{},\"n\":{},\"observations\":[{rows}]}}",
            js(basis.basis.code()), basis.sd, basis.warning_limit, basis.failure_limit, basis.warnings, basis.failures, result.n));
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} | {} |",
            basis.basis.code(),
            basis.sd,
            basis.warning_limit,
            basis.failure_limit,
            basis.warnings,
            basis.failures,
            result.n
        );
    }
    let bias = &result.bias;
    let canmet = &result.canmet;
    let _ = write!(
        markdown,
        "\nSD bases: sigma_c is certified within-lab; sigma_l is certified between-lab; lab_running_sw is the supplied lab running SD. Exceedance is strict on computed binary64 values without an extra rounding tolerance; equality passes the corresponding threshold.\n\n**CANMET:** {}. Absolute bias {} versus limit {} = 2 × sqrt(sigma_l² + lab_running_sw² / N).\n\n**Signed bias:** {} (magnitude {}), with a {:.3}% Student t interval [{}, {}], {} degrees of freedom. This interval conditions on a fixed certified mean and independent normal replicates; it does not include the certificate's uncertainty.\n\n| Sample ID | Value | Deviation | sigma_c | sigma_l | lab_running_sw | Source line |\n|---|---:|---:|---|---|---|---:|\n",
        if canmet.passed { "pass" } else { "failure" },
        canmet.absolute_bias,
        canmet.limit,
        bias.signed_bias,
        bias.magnitude,
        100.0 * bias.confidence,
        bias.lower,
        bias.upper,
        bias.degrees_of_freedom
    );
    for (index, (id, line)) in input.observations.iter().enumerate() {
        let _ = writeln!(
            markdown,
            "| {} | {} | {} | {} | {} | {} | {line} |",
            worksheet_input::markdown_cell(id),
            input.batch.values[index],
            result.single_value[0].results[index].deviation,
            state(&result.single_value[0].results[index].state),
            state(&result.single_value[1].results[index].state),
            state(&result.single_value[2].results[index].state)
        );
    }
    let _ = write!(
        markdown,
        "\nDeclared certificate source: {}. Verification: **not performed**.\n\nOmitted uncertainty: {}\n\nSource: `{}`; SHA-256: `{}`.\n\n{NO_CLAIM}\n",
        worksheet_input::markdown_cell(&input.certificate_source),
        worksheet_input::markdown_cell(&input.omitted_uncertainty),
        worksheet_input::markdown_cell(&source.artifact_path),
        Digest32(source.artifact_sha256)
    );
    let report = format!(
        "{{\"schema\":\"fmine.crm-batch-report.v1\",\"crm_id\":{},\"lab\":{},\"analyte\":{},\"method\":{},\"unit\":{},\"evidence_lane\":{},\"certificate\":{{\"source\":{},\"verification\":\"not_performed\"}},\"source\":{},\"omitted_uncertainty\":{},\"declarations\":{{\"certified_mean\":{},\"sigma_c\":{},\"sigma_l\":{},\"lab_running_sw\":{},\"warning_k\":{},\"failure_k\":{},\"confidence\":{}}},\"threshold_comparison\":\"strict_binary64_exceedance\",\"warning_counts_include_failures\":true,\"n\":{},\"mean\":{},\"sample_sd\":{},\"standard_error\":{},\"single_value\":[{}],\"canmet\":{{\"limit\":{},\"absolute_bias\":{},\"passed\":{}}},\"bias\":{{\"signed_bias\":{},\"magnitude\":{},\"lower\":{},\"upper\":{},\"confidence\":{},\"degrees_of_freedom\":{},\"t_critical\":{},\"basis\":\"student_t_independent_normal_fixed_reference\"}}}}",
        js(&input.crm_id),
        js(&input.lab),
        js(&input.analyte),
        js(&input.method),
        js(&input.unit),
        js(&input.lane),
        js(&input.certificate_source),
        worksheet_input::source_json(source, bytes),
        js(&input.omitted_uncertainty),
        input.batch.certified_mean,
        input.batch.sigma_c,
        input.batch.sigma_l,
        input.batch.lab_running_sw,
        input.policy.warning_k,
        input.policy.failure_k,
        input.policy.confidence,
        result.n,
        result.mean,
        result.sample_sd,
        result.standard_error,
        bases.join(","),
        canmet.limit,
        canmet.absolute_bias,
        canmet.passed,
        bias.signed_bias,
        bias.magnitude,
        bias.lower,
        bias.upper,
        bias.confidence,
        bias.degrees_of_freedom,
        bias.t_critical,
    );
    (report, markdown)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(&command.input), "CRM batch", MAX_BYTES)?;
        let input = declaration(&text)?;
        let result = score_crm_batch(&input.batch, &input.policy).map_err(|e| e.to_refusal())?;
        let (report, mut markdown) = render(&input, &result, &source, text.len());
        let interval = Representation::Interval {
            lo: result.bias.lower,
            hi: result.bias.upper,
        };
        let claim = Claim::new(
            "drill.crm-batch/1",
            result,
            &report,
            interval,
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
                "inspect the CRM batch source and policy contract",
            )],
        )
    });
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
