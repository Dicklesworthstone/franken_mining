//! DESIGN (BD-167, BD-284): source-bound blank and duplicate diagnostics using the Corebox engine.
//! Determinism: BitwisePortable (ordered source rows, explicit policy and source hashes).

mod blanks;
mod duplicates;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_drill::records::AssayValue;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_registry::toml_subset::{TomlField, TomlValue};

use crate::{Envelope, NextAction, journal_arguments, json_string as js, worksheet_input};

pub const BLANKS_VERB: &str = "drill blanks";
pub const DUPLICATES_VERB: &str = "drill duplicates";
pub const BLANKS_USAGE: &str = "drill blanks --input <blanks.toml>";
pub const DUPLICATES_USAGE: &str = "drill duplicates --input <duplicates.toml>";
pub use blanks::ROBOT_DOCS_JSON as BLANKS_DOCS;
pub use duplicates::ROBOT_DOCS_JSON as DUPLICATES_DOCS;
pub const NO_CLAIM: &str = "Diagnostics of supplied observations under declared policy only. Inputs, source descriptions and policy approval are unverified; outputs are Assumed, Internal and uncalibrated. Censored results retain their bounds and unavailable statistics remain unavailable. No aggregate QA/QC acceptance, certificate authentication, custody, assay release, compliance or execution authority is established.";
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_ROWS: usize = 4096;
const COMMON: &[&str] = &[
    "schema",
    "analyte",
    "method",
    "unit",
    "basis",
    "evidence_lane",
    "source_description",
    "omitted_uncertainty",
    "policy_id",
    "policy_version",
    "policy_source",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Blanks,
    Duplicates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
    pub kind: Kind,
}

impl Kind {
    fn verb(self) -> &'static str {
        match self {
            Self::Blanks => BLANKS_VERB,
            Self::Duplicates => DUPLICATES_VERB,
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Blanks => BLANKS_USAGE,
            Self::Duplicates => DUPLICATES_USAGE,
        }
    }
}

pub fn parse(args: &[String], kind: Kind) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    if words.join(" ") != kind.verb() || !options.is_empty() {
        return Err(kind.usage().into());
    }
    Ok(Command {
        input: input.ok_or(kind.usage())?.into(),
        kind,
    })
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill quality declaration: {}", detail.into()),
    )
}

fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
) -> Result<BTreeMap<&'a str, &'a str>, Refusal> {
    let mut result = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
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

fn number(token: &str) -> Result<f64, Refusal> {
    let value = token
        .parse::<f64>()
        .map_err(|_| invalid(format!("invalid numeric token {token}")))?;
    let nonzero = token
        .split(['e', 'E'])
        .next()
        .unwrap_or("")
        .bytes()
        .any(|b| matches!(b, b'1'..=b'9'));
    if !value.is_finite() || (value == 0.0 && nonzero) {
        return Err(invalid("nonfinite or underflowing numeric token"));
    }
    Ok(value)
}

fn assay(token: &str) -> Result<AssayValue, Refusal> {
    let (prefix, rest) = if let Some(rest) = token.strip_prefix('<') {
        ('<', rest)
    } else if let Some(rest) = token.strip_prefix('>') {
        ('>', rest)
    } else {
        ('=', token)
    };
    let value = number(rest)?;
    if !(0.0..=1.0).contains(&value)
        || (prefix != '=' && value == 0.0)
        || (prefix == '>' && value == 1.0)
    {
        return Err(invalid(
            "assay mass fractions require [0,1], <DL requires (0,1], and >UL requires (0,1)",
        ));
    }
    Ok(match prefix {
        '<' => AssayValue::BelowDetection { limit: value },
        '>' => AssayValue::AboveUpper { limit: value },
        _ => AssayValue::Measured {
            mass_fraction: value,
        },
    })
}

fn assay_json(value: AssayValue) -> String {
    match value {
        AssayValue::Measured { mass_fraction } => {
            format!("{{\"kind\":\"measured\",\"value\":{mass_fraction}}}")
        }
        AssayValue::BelowDetection { limit } => {
            format!("{{\"kind\":\"below_detection\",\"limit\":{limit},\"substitution\":null}}")
        }
        AssayValue::AboveUpper { limit } => {
            format!("{{\"kind\":\"above_upper\",\"limit\":{limit},\"substitution\":null}}")
        }
    }
}

fn optional_number(value: Option<f64>) -> String {
    value.map_or_else(|| "null".into(), |v| v.to_string())
}

fn common(top: &BTreeMap<&str, &str>, schema: &str) -> Result<String, Refusal> {
    if top["schema"] != schema {
        return Err(invalid(format!("schema must be {schema}")));
    }
    if top["unit"] != "mass_fraction" || !matches!(top["basis"], "dry" | "wet") {
        return Err(invalid(
            "declare unit mass_fraction and basis dry or wet; conversion is never implicit",
        ));
    }
    if !matches!(top["evidence_lane"], "synthetic" | "internal") {
        return Err(invalid("evidence_lane must be synthetic or internal"));
    }
    Ok(COMMON
        .iter()
        .filter(|key| **key != "schema")
        .map(|key| format!("{}:{}", js(key), js(top[key])))
        .collect::<Vec<_>>()
        .join(","))
}

fn heading(title: &str, top: &BTreeMap<&str, &str>) -> String {
    let cell = worksheet_input::markdown_cell;
    format!(
        "# {title}\n\nAnalyte **{}** · method **{}** · mass fraction, {} basis · evidence lane {}.\n\nPolicy **{}@{}**, declared source {}.\n\nOmitted uncertainty: {}\n\n",
        cell(top["analyte"]),
        cell(top["method"]),
        top["basis"],
        top["evidence_lane"],
        cell(top["policy_id"]),
        cell(top["policy_version"]),
        cell(top["policy_source"]),
        cell(top["omitted_uncertainty"])
    )
}

fn finish<T>(
    kind: Kind,
    result: T,
    report: String,
    mut markdown: String,
    source: SourceAnchor,
) -> Result<Envelope, Refusal> {
    let metric = match kind {
        Kind::Blanks => "drill.blank-batch/1",
        Kind::Duplicates => "drill.duplicate-batch/1",
    };
    let claim = Claim::new(
        metric,
        result,
        &report,
        Representation::Placeholder {
            token: "diagnostic_report_no_aggregate_score".into(),
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
        "\nSource: `{}`; SHA-256 `{}`.\n\nClaim `{}`; receipt SHA-256 `{receipt}`.\n\n{NO_CLAIM}\n",
        worksheet_input::markdown_cell(&source.artifact_path),
        Digest32(source.artifact_sha256),
        claim.claim_id
    );
    let mut envelope = Envelope::ok(
        kind.verb(),
        format!(
            "{{\"receipt_sha256\":\"{receipt}\",\"claim\":{{\"id\":{},\"metric\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"confidentiality\":\"Internal\",\"calibration\":\"uncalibrated\",\"support\":\"point_assay_samples\",\"additivity\":\"NonAdditive\",\"aggregate_score\":null}},\"report\":{report}}}",
            js(&claim.claim_id.to_string()),
            js(metric)
        ),
        vec![],
    );
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(&command.input), command.kind.verb(), MAX_BYTES)?;
        match command.kind {
            Kind::Blanks => blanks::evaluate(&text, source),
            Kind::Duplicates => duplicates::evaluate(&text, source),
        }
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            command.kind.verb(),
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the declared QA/QC source and policy contract",
            )],
        )
    });
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
