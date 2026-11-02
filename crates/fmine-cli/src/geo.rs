//! Exact production GEO comparisons under explicitly declared versioned price decks (BD-062).
//! Determinism: Exact

use std::path::{Path, PathBuf};

use fmine_core::Digest32;
use fmine_evidence::Refusal;
use fmine_metrics::definitions::geo::{self, worksheet};

use crate::{Envelope, NextAction, escape_json, worksheet_input};

const MAX_BYTES: u64 = 1024 * 1024;
pub const USAGE: &str = "metrics geo --input <worksheet.toml>";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.geo-worksheet.v1","input":"Regular UTF-8 TOML, at most 1 MiB; exact submitted bytes are SHA-256 bound","fields":["schema","issuer","subject","period","ownership","basis","population","currency","metals","reference_deck","reference_version"],"optional_fields":["ownership_share"],"quantity_rows":["metal","unit","amount"],"deck_rows":["id","version","basis","currency","gold_price_per_oz"],"optional_deck_fields":["copper_price_per_lb","silver_price_per_oz"],"values":"Quoted strings; exact decimal or n/d amounts; period YYYYFY/YYYYQn/YYYYHn; full ownership without a share or attributable ownership with a share; comma-separated metals gold,copper,silver; gold and silver quantities use oz, copper uses lb","population":"Every declared metal requires exactly one quantity and its positive price in every deck; omitted metals are explicitly outside the population","decks":"Two to sixty-four named/versioned decks, including the explicit reference; currency matches the physical scope; no implicit mass, ownership or FX conversion","output":"Exact GEO ounces, each metal contribution and differences from the reference on unchanged physical quantities, canonical input/output Claims and digest","evidence":"Assumed, Internal, uncalibrated; source bytes are bound but the declarations are not independently verified","example":"crates/xtask/testdata/production_geo.toml"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
}

/// Common output/workspace options are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("metrics")
        || args.get(1).map(String::as_str) != Some("geo")
    {
        return Err(USAGE.into());
    }
    let mut input = None;
    let mut i = 2;
    while i < args.len() {
        if args[i] != "--input" {
            return Err(format!(
                "metrics geo: unsupported option {}; {USAGE}",
                args[i]
            ));
        }
        if input.is_some() {
            return Err("metrics geo: duplicate --input".into());
        }
        let value = args
            .get(i + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or("metrics geo: missing value for --input")?;
        input = Some(PathBuf::from(value));
        i += 2;
    }
    Ok(Command {
        input: input.ok_or("metrics geo requires --input")?,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(&command.input), "GEO worksheet", MAX_BYTES)?;
        let receipt = worksheet::evaluate(&text, source.clone())?;
        let payload = format!(
            "{{\"schema\":\"{}\",\"mode\":\"declared_production_geo\",\"source\":{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{}}},\"receipt\":{}}}",
            worksheet::SCHEMA,
            escape_json(&source.artifact_path),
            Digest32(source.artifact_sha256),
            text.len(),
            receipt.to_json()
        );
        let mut envelope = Envelope::ok("metrics geo", payload, Vec::new());
        envelope.no_claim = Some(geo::NO_CLAIM);
        envelope.warnings.push(
            "Declared production quantities and price decks are Assumed, Internal and uncalibrated.".into(),
        );
        envelope.rendered_markdown = Some(receipt.to_markdown());
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "metrics geo",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the production-GEO worksheet population, units and price-deck requirements",
            )],
        )
    })
}
