//! Exact declared cost worksheets over the workspace's registered AISC/AIC definitions (BD-061).
//! Determinism: Exact

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::Refusal;
use fmine_metrics::definitions::{claims, worksheet};

use crate::{Envelope, NextAction, escape_json, worksheet_input};

const MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const USAGE: &str = "metrics cost --input <worksheet.toml> --definition <registered-id>";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.cost-worksheet.v1","input":"Regular UTF-8 TOML, at most 4 MiB; actual worksheet and workspace metric registry bytes are SHA-256 bound","fields":["schema","issuer","subject","period","ownership","basis","currency","complete","ounces_sold"],"optional_fields":["ownership_share","gold_share"],"cost_rows":["item","usd"],"uu_rows":["id","description","trigger","downside_usd_m"],"values":"Quoted strings; complete must be true; period YYYYFY/YYYYQn/YYYYHn; full or attributable ownership; USD amounts at most four fractional digits; quantities and shares exact decimals or n/d","definition":"Explicit registered AISC/AIC ID; effective inherited lines/exclusions and successor retained","output":"Exact numerator, gold ounces sold, USD/oz, signed contribution per input and complete canonical Claim receipts","evidence":"Assumed, Internal, uncalibrated; completeness is a declared assumption","example":"tests/costs/declared_aisc.toml"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
    pub definition: String,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("metrics")
        || args.get(1).map(String::as_str) != Some("cost")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if !matches!(flag, "--input" | "--definition") {
            return Err(format!("metrics cost: unsupported option {flag}; {USAGE}"));
        }
        let value = args
            .get(i + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or_else(|| format!("metrics cost: missing value for {flag}"))?;
        if fields.insert(flag, value).is_some() {
            return Err(format!("metrics cost: duplicate {flag}"));
        }
        i += 2;
    }
    Ok(Command {
        input: PathBuf::from(
            fields
                .get("--input")
                .ok_or("metrics cost requires --input")?
                .as_str(),
        ),
        definition: (*fields
            .get("--definition")
            .ok_or("metrics cost requires --definition")?)
        .clone(),
    })
}

fn read(path: &Path) -> Result<(String, SourceAnchor), Refusal> {
    worksheet_input::read(path, "cost worksheet", MAX_BYTES)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = read(&root.join(&command.input))?;
        let (metrics, registry) = read(&root.join("registries/metrics.toml"))?;
        let receipt = worksheet::evaluate(
            &text,
            source.clone(),
            &metrics,
            registry.clone(),
            &command.definition,
        )?;
        let source_json = |anchor: &SourceAnchor, bytes: usize| {
            format!(
                "{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{bytes}}}",
                escape_json(&anchor.artifact_path),
                fmine_core::Digest32(anchor.artifact_sha256).to_hex()
            )
        };
        let payload = format!(
            "{{\"schema\":\"{}\",\"mode\":\"declared_cost_worksheet\",\"source\":{},\"registry\":{},\"receipt\":{}}}",
            worksheet::SCHEMA,
            source_json(&source, text.len()),
            source_json(&registry, metrics.len()),
            receipt.to_json()
        );
        let mut envelope = Envelope::ok("metrics cost", payload, Vec::new());
        envelope.no_claim = Some(claims::NO_CLAIM);
        envelope.warnings.push("Declared costs, classification and completeness are Assumed, Internal and uncalibrated.".into());
        envelope.rendered_markdown = Some(receipt.to_markdown());
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "metrics cost",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the cost-worksheet input schema and selected definition requirements",
            )],
        )
    })
}
