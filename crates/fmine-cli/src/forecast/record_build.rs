//! DESIGN (BD-067/334): prepare a source-bound empirical candidate without issuing a forecast record.
//! The model and evidence cut are explicit; author attestations and external time proofs are never inferred.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

use fmine_calib::record::candidate::{CandidatePool, ExcludedPair, MODEL_ID};
use fmine_calib::record::{ForecastRow, Outcome, Rat, Target, parse_utc};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::forecast::{CandidateObservation, CandidateSource, PublicCandidate};

use crate::{Envelope, NextAction, OutputFormat, json_string as js};

pub const USAGE: &str = "forecast record build --rules <rules.toml> --model pooled-quarterly-ratio-empirical-v1 --as-of YYYY-MM-DDTHH:MM:SSZ [--decimal-places 6]";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.forecast-record-candidate.v1","model":"pooled-quarterly-ratio-empirical-v1","status":"draft; uncalibrated; not issued or externally timestamped","targets":{"fy_minus_9m":["au_prod_100_koz","cu_prod_100_kt"],"q4_qtd":["aisc_usd_per_oz","c1_usd_per_lb","aisc_usd_per_lb"]},"inputs":"Explicit rules and evidence cut; only required original Mine Stats releases with verified hashes and semantic source Claims","estimate":"Latest Q3 observation multiplied by the pooled historical quarterly-ratio median","interval":"Latest Q3 observation multiplied by inverse empirical CDF tails under the rules' alpha; exact bounds equal the registered persistence baseline","rounding":"CSV lower bounds round down, upper bounds up, points to nearest with ties upward; 0 through 12 decimal places, default 6","outputs":"Complete live-site/target forecasts_csv, exact and serialized rows, excluded transitions, source Claims, input manifest and deterministic receipt","missing":"Missing required sources or live point observations refuse; no comparative substitution or silent population shrinkage","issuance":"Does not change files, supply an author attestation, freeze a forecast or obtain a timestamp; review and use the existing record workflow"}"#;

const VERB: &str = "forecast record build";
const MAX_RULES_BYTES: u64 = 1024 * 1024;
const NO_CLAIM: &str = "Uncalibrated draft under an explicitly selected empirical model. Historical pooled ratios do not establish independent, stationary or calibrated future outcomes. Exact interval bounds equal the registered persistence baseline; decimal export may widen them. This does not issue or time-prove a forecast, supply an author attestation, or establish forecasting skill. From registered public evidence; no affiliation implied.";
const SYNTHETIC_NO_CLAIM: &str = "Synthetic, uncalibrated draft from invented source workbooks. It demonstrates the empirical calculation and does not describe an actual issuer or establish forecasting skill. Exact interval bounds equal the persistence baseline on the same synthetic inputs; decimal export may widen them. This does not issue or time-prove a forecast or supply an author attestation.";

fn no_claim(synthetic: bool) -> &'static str {
    if synthetic {
        SYNTHETIC_NO_CLAIM
    } else {
        NO_CLAIM
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub rules: PathBuf,
    pub as_of: String,
    pub decimal_places: u32,
}

pub(crate) fn parse(
    args: &[String],
    default_root: &Path,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    let mut values = BTreeMap::new();
    let mut words = Vec::new();
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--json" if !json => json = true,
            "--rules" | "--as-of" | "--model" | "--decimal-places" | "--workspace" | "--format" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| format!("{arg} needs a value"))?;
                if value.is_empty()
                    || value.starts_with("--")
                    || value.chars().any(char::is_control)
                {
                    return Err(format!(
                        "{arg} needs a nonempty value without control characters"
                    ));
                }
                if values.insert(arg, value.as_str()).is_some() {
                    return Err(format!("duplicate option {arg}"));
                }
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown or repeated option {flag}"));
            }
            word => words.push(word),
        }
        i += 1;
    }
    if words != ["forecast", "record", "build"] {
        return Err(format!("expected {USAGE}"));
    }
    let required = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("{VERB} requires {key}"))
    };
    if required("--model")? != MODEL_ID {
        return Err(format!("--model requires {MODEL_ID}"));
    }
    let as_of = required("--as-of")?;
    if parse_utc(as_of).is_none() {
        return Err("--as-of requires YYYY-MM-DDTHH:MM:SSZ".into());
    }
    let decimal_places = values
        .get("--decimal-places")
        .copied()
        .unwrap_or("6")
        .parse::<u32>()
        .ok()
        .filter(|n| *n <= 12)
        .ok_or("--decimal-places requires an integer from 0 through 12")?;
    let format = match values.get("--format").copied() {
        None | Some("json") => OutputFormat::Json,
        Some("markdown" | "md") if !json => OutputFormat::Markdown,
        Some("markdown" | "md") => return Err("--json conflicts with Markdown format".into()),
        _ => return Err("--format requires json|markdown".into()),
    };
    Ok((
        Command {
            rules: PathBuf::from(required("--rules")?),
            as_of: as_of.into(),
            decimal_places,
        },
        format,
        values
            .get("--workspace")
            .map_or_else(|| default_root.to_path_buf(), |p| PathBuf::from(*p)),
    ))
}

#[must_use]
pub fn execute(command: &Command, root: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Review draft generation, source requirements and the separate frozen-record workflow",
    )];
    let result = read_rules(&root.join(&command.rules)).and_then(|bytes| {
        fmine_public::forecast::build(root, &bytes, &command.as_of, command.decimal_places)
    });
    match result {
        Ok(report) => {
            let mut envelope = Envelope::ok(VERB, render_json(&report), actions);
            envelope.no_claim = Some(no_claim(report.synthetic));
            envelope.rendered_markdown = Some(render_markdown(&report));
            envelope
        }
        Err(refusal) => Envelope::refused(VERB, refusal, actions),
    }
}

fn read_rules(path: &Path) -> Result<Vec<u8>, Refusal> {
    let unreadable = |e: std::io::Error| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("forecast candidate rules {}: {e}", path.display()),
        )
    };
    let metadata = std::fs::metadata(path).map_err(unreadable)?;
    if !metadata.is_file() || metadata.len() > MAX_RULES_BYTES {
        return Err(super::invalid(
            "candidate rules must be a regular file no larger than 1 MiB",
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(unreadable)?
        .take(MAX_RULES_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_RULES_BYTES {
        return Err(super::invalid(
            "candidate rules grew beyond their byte limit",
        ));
    }
    Ok(bytes)
}

fn exact(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), js)
}

fn row_json(row: &ForecastRow) -> String {
    format!(
        "{{\"site\":{},\"target\":{},\"point\":{},\"lower\":{},\"upper\":{},\"uncalibrated\":true}}",
        js(&row.site),
        js(&row.target),
        js(&exact(row.point)),
        js(&exact(row.lo)),
        js(&exact(row.hi)),
    )
}

fn target_json(target: &Target) -> String {
    use fmine_calib::record::TargetMetric;
    let metric = TargetMetric::from_target(target);
    let unit = metric.map(TargetMetric::unit);
    let basis = match metric {
        Some(TargetMetric::GoldProduction | TargetMetric::CopperProduction) => Some("100%"),
        Some(TargetMetric::GoldAisc) => Some("reported_rate_per_ounce_sold"),
        Some(TargetMetric::CopperC1 | TargetMetric::CopperAisc) => {
            Some("reported_rate_per_copper_pound_sold")
        }
        None => None,
    };
    let outcome = match target.outcome {
        Outcome::FyMinus9m => "fy_minus_9m",
        Outcome::Q4Qtd => "q4_qtd",
    };
    format!(
        "{{\"id\":{},\"field\":{},\"quantity\":{},\"scored_quantity\":{},\"unit\":{},\"basis\":{},\"outcome\":{}}}",
        js(&target.id),
        js(&target.field),
        js(&target.quantity),
        js(&target.scored_quantity),
        optional(unit),
        optional(basis),
        js(outcome),
    )
}

fn source_json(source: &CandidateSource) -> String {
    format!(
        "{{\"file\":{},\"release\":{},\"url\":{},\"sha256\":{},\"bytes\":{},\"retrieved_utc\":{},\"available_by_utc\":{},\"published_at\":{},\"published_at_basis\":{}}}",
        js(&source.file),
        js(&source.release),
        js(&source.url),
        js(&source.sha256),
        source.bytes,
        js(&source.retrieved_utc),
        js(&source.available_by_utc),
        optional(source.published_at.as_deref()),
        optional(source.published_at_basis.as_deref()),
    )
}

fn observation_json(observation: &CandidateObservation) -> String {
    format!(
        "{{\"site\":{},\"target\":{},\"quarter\":{},\"role\":{},\"source\":{},\"sha256\":{},\"value\":{},\"reason\":{},\"claim\":{}}}",
        js(&observation.site),
        js(&observation.target),
        js(&observation.quarter.to_string()),
        js(observation.role),
        js(&observation.source),
        js(&observation.sha256),
        observation
            .value
            .map_or_else(|| "null".into(), |value| js(&exact(value))),
        optional(observation.reason.as_deref()),
        observation
            .claim
            .as_ref()
            .map_or_else(|| "null".into(), |claim| claim.to_json()),
    )
}

fn excluded_json(pair: &ExcludedPair) -> String {
    let number = |value: Option<Rat>| value.map_or_else(|| "null".into(), |v| js(&exact(v)));
    format!(
        "{{\"site\":{},\"from\":{},\"to\":{},\"before\":{},\"after\":{}}}",
        js(&pair.site),
        js(&pair.from.to_string()),
        js(&pair.to.to_string()),
        number(pair.before),
        number(pair.after),
    )
}

fn pool_json(pool: &CandidatePool) -> String {
    format!(
        "{{\"target\":{},\"from\":{},\"through\":{},\"point_quarter\":{},\"ratio_count\":{},\"ratios\":[{}],\"excluded_pairs\":[{}],\"lower_ratio\":{},\"median_ratio\":{},\"upper_ratio\":{}}}",
        js(&pool.target),
        js(&pool.pool_from.to_string()),
        js(&pool.pool_through.to_string()),
        js(&pool.point_quarter.to_string()),
        pool.ratios.len(),
        pool.ratios
            .iter()
            .map(|value| js(&exact(*value)))
            .collect::<Vec<_>>()
            .join(","),
        pool.excluded_pairs
            .iter()
            .map(excluded_json)
            .collect::<Vec<_>>()
            .join(","),
        js(&exact(pool.lower_ratio)),
        js(&exact(pool.median_ratio)),
        js(&exact(pool.upper_ratio)),
    )
}

fn render_json(report: &PublicCandidate) -> String {
    let candidate = &report.candidate;
    format!(
        "{{\"schema\":\"fmine.forecast-record-candidate.v1\",\"status\":\"draft\",\"model\":{},\"issuer\":{},\"record_id\":{},\"as_of\":{},\"synthetic\":{},\"uncalibrated\":true,\"issued\":false,\"attestation\":null,\"timestamp_proof\":null,\"alpha\":{},\"decimal_places\":{},\"rounding\":\"lower_down_upper_up_point_nearest_ties_up\",\"rules_digest\":{},\"rules_file_sha256\":{},\"registry_sha256\":{},\"receipt_digest\":{},\"forecasts_csv\":{},\"targets\":[{}],\"exact_forecasts\":[{}],\"serialized_forecasts\":[{}],\"not_published_sites\":[{}],\"pools\":[{}],\"input_manifest\":[{}],\"observations\":[{}]}}",
        js(candidate.model_id),
        js(&report.rules.issuer),
        js(&report.rules.record_id),
        js(&report.as_of),
        report.synthetic,
        js(&exact(candidate.alpha)),
        candidate.decimal_places,
        js(&report.rules_digest.to_hex()),
        js(&report.rules_file_sha256.to_hex()),
        js(&report.registry_sha256.to_hex()),
        js(&report.digest.to_hex()),
        js(&candidate.csv),
        report
            .rules
            .targets
            .iter()
            .map(target_json)
            .collect::<Vec<_>>()
            .join(","),
        candidate
            .exact_rows
            .iter()
            .map(row_json)
            .collect::<Vec<_>>()
            .join(","),
        candidate
            .serialized_rows
            .iter()
            .map(row_json)
            .collect::<Vec<_>>()
            .join(","),
        candidate
            .not_published_sites
            .iter()
            .map(|site| js(site))
            .collect::<Vec<_>>()
            .join(","),
        candidate
            .pools
            .iter()
            .map(pool_json)
            .collect::<Vec<_>>()
            .join(","),
        report
            .sources
            .iter()
            .map(source_json)
            .collect::<Vec<_>>()
            .join(","),
        report
            .observations
            .iter()
            .map(observation_json)
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn render_markdown(report: &PublicCandidate) -> String {
    let mut out = format!(
        "# Draft forecast candidate\n\nModel: `{}`. Evidence cut: `{}`. Calibration: **uncalibrated**.\n\nEvidence lane: **{}**.\n\n{}\n\n## Targets\n\n",
        report.candidate.model_id,
        report.as_of,
        if report.synthetic {
            "synthetic"
        } else {
            "registered public sources"
        },
        no_claim(report.synthetic),
    );
    for target in &report.rules.targets {
        let _ = writeln!(
            out,
            "- **{}**: {}; {}.",
            target.id, target.quantity, target.scored_quantity
        );
    }
    out.push_str("\n## Exact model values\n\n| Site | Target | Median | Lower | Upper |\n|---|---|---:|---:|---:|\n");
    for row in &report.candidate.exact_rows {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            row.site.replace('|', "\\|"),
            row.target.replace('|', "\\|"),
            exact(row.point),
            exact(row.lo),
            exact(row.hi)
        );
    }
    out.push_str("\n## Historical pools\n\n");
    for pool in &report.candidate.pools {
        let _ = writeln!(
            out,
            "- {}: {} ratios, {} excluded adjacent pairs, {} through {}; latest observation {}.",
            pool.target,
            pool.ratios.len(),
            pool.excluded_pairs.len(),
            pool.pool_from,
            pool.pool_through,
            pool.point_quarter
        );
    }
    let _ = write!(
        out,
        "\n## Complete candidate CSV\n\nBounds round outward to {} decimal places; points round to nearest with ties upward.\n\n```csv\n{}```\n\n## Source manifest\n\n",
        report.candidate.decimal_places, report.candidate.csv,
    );
    for source in &report.sources {
        let _ = writeln!(
            out,
            "- `{}` ({}): SHA-256 `{}`; available by `{}`.",
            source.file, source.release, source.sha256, source.available_by_utc
        );
    }
    if !report.candidate.not_published_sites.is_empty() {
        let _ = writeln!(
            out,
            "\nNot published under the rules: {}.",
            report.candidate.not_published_sites.join(", ")
        );
    }
    let _ = writeln!(
        out,
        "\nCandidate receipt: `{}`. The JSON view retains every source observation and Claim.",
        report.digest.to_hex()
    );
    out
}
