//! DESIGN (BD-076): score the exact FMREC1 record under its previously stamped rules. The receipt trust
//! roots are an explicit caller input; source availability uses frozen retrieval times and registered
//! publication dates, with a labelled retrieval fallback, rather than an implicit host clock.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};

use fmine_calib::record::{self, ForecastRow, RecordManifest, RecordRules, parse_hex32, parse_utc};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::forecast::timestamp::{VerifiedTimestamp, verify_rfc3161};

use crate::{Envelope, NextAction, OutputFormat};

use super::{invalid, js, record_sources};

const VERB: &str = "forecast record score";
const SCHEMA: &str = "fmine.forecast-record-assessment.v1";
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const NO_CLAIM: &str = "Descriptive scores of a frozen supplied forecast record under its registered rules. RFC 3161 signatures are checked against caller-selected trust roots; this does not establish forecasting skill, independently authenticate issuer statements, or issue a new forecast. From public filings; no affiliation implied.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub record: PathBuf,
    pub as_of: String,
    pub tsa_roots: PathBuf,
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
            "--record" | "--as-of" | "--tsa-roots" | "--workspace" | "--format" => {
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
    if words != ["forecast", "record", "score"] {
        return Err(format!("expected {VERB}"));
    }
    let required = |key| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| format!("{VERB} requires {key}"))
    };
    let as_of = required("--as-of")?;
    if parse_utc(as_of).is_none() {
        return Err("--as-of requires YYYY-MM-DDTHH:MM:SSZ".into());
    }
    let format = match values.get("--format").copied() {
        None | Some("json") => OutputFormat::Json,
        Some("markdown" | "md") if !json => OutputFormat::Markdown,
        Some("markdown" | "md") => return Err("--json conflicts with Markdown format".into()),
        _ => return Err("--format requires json|markdown".into()),
    };
    let workspace = values
        .get("--workspace")
        .map_or_else(|| default_root.to_path_buf(), |path| PathBuf::from(*path));
    Ok((
        Command {
            record: PathBuf::from(required("--record")?),
            as_of: as_of.into(),
            tsa_roots: PathBuf::from(required("--tsa-roots")?),
        },
        format,
        workspace,
    ))
}

#[must_use]
pub fn execute(command: &Command, root: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Read the frozen-record scoring command and its explicit source and time-proof requirements",
    )];
    match perform(command, root) {
        Ok((payload, markdown)) => {
            let mut envelope = Envelope::ok(VERB, payload, actions);
            envelope.no_claim = Some(NO_CLAIM);
            envelope.rendered_markdown = Some(markdown);
            envelope
        }
        Err(refusal) => Envelope::refused(VERB, refusal, actions),
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Refusal> {
    let failure = |error: std::io::Error| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("forecast record {}: {error}", path.display()),
        )
    };
    let metadata = std::fs::metadata(path).map_err(failure)?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(invalid(
            "forecast record inputs must be bounded regular files",
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(failure)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(invalid("forecast record input grew beyond its byte limit"));
    }
    Ok(bytes)
}

fn text(bytes: &[u8]) -> Result<&str, Refusal> {
    std::str::from_utf8(bytes).map_err(|_| invalid("forecast record text must be UTF-8"))
}

fn checked_digest(path: &Path, expected: Digest32) -> Result<(), Refusal> {
    let bytes = read(path)?;
    let stored = text(&bytes)?.trim_end_matches('\n');
    if parse_hex32(stored) != Some(expected.0) {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!("{} differs from the exact record bytes", path.display()),
        ));
    }
    Ok(())
}

struct Loaded {
    rules: RecordRules,
    manifest: RecordManifest,
    forecasts: Vec<ForecastRow>,
    record_digest: Digest32,
    rules_digest: Digest32,
}

fn load(dir: &Path) -> Result<Loaded, Refusal> {
    let rules_bytes = read(&dir.join("rules.toml"))?;
    let manifest_bytes = read(&dir.join("manifest.toml"))?;
    let forecasts_bytes = read(&dir.join("forecasts.csv"))?;
    let rules_text = text(&rules_bytes)?;
    if rules_text.lines().any(record::has_open_marker) {
        return Err(invalid("frozen rules contain an unresolved OPEN marker"));
    }
    let rules = record::parse_rules(rules_text).map_err(|e| invalid(e.to_string()))?;
    let manifest =
        record::parse_manifest(text(&manifest_bytes)?).map_err(|e| invalid(e.to_string()))?;
    let forecasts =
        record::parse_forecasts(&forecasts_bytes, &rules).map_err(|e| invalid(e.to_string()))?;
    let rules_digest = record::rules_digest(&rules_bytes);
    let record_digest = record::record_digest(&manifest_bytes, &forecasts_bytes);
    if manifest.record_id != rules.record_id || manifest.rules_sha256 != rules_digest.0 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "the record manifest does not bind these exact rules and record id",
        ));
    }
    checked_digest(&dir.join("rules.sha256"), rules_digest)?;
    checked_digest(&dir.join("record.sha256"), record_digest)?;
    Ok(Loaded {
        rules,
        manifest,
        forecasts,
        record_digest,
        rules_digest,
    })
}

fn proof_json(proof: &VerifiedTimestamp) -> String {
    format!(
        "{{\"kind\":\"rfc3161\",\"verification\":\"signature_and_time_against_explicit_roots\",\"lower_utc\":{},\"upper_utc\":{},\"receipt_sha256\":{},\"trust_roots_sha256\":{}}}",
        js(&record::format_utc(proof.times.lower_utc)),
        js(&record::format_utc(proof.times.upper_utc)),
        js(&proof.receipt_sha256.to_hex()),
        js(&proof.trust_roots_sha256.to_hex()),
    )
}

fn perform(command: &Command, root: &Path) -> Result<(String, String), Refusal> {
    let as_of =
        parse_utc(&command.as_of).ok_or_else(|| invalid("invalid assessment information cut"))?;
    let dir = root.join(&command.record);
    let loaded = load(&dir)?;
    let roots = root.join(&command.tsa_roots);
    let rules_proof = verify_rfc3161(&dir.join("rules.tsr"), &roots, loaded.rules_digest)?;
    let record_proof = verify_rfc3161(&dir.join("record.tsr"), &roots, loaded.record_digest)?;
    if rules_proof.trust_roots_sha256 != record_proof.trust_roots_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "timestamp trust roots changed during verification",
        ));
    }
    if rules_proof.times.upper_utc >= record_proof.times.lower_utc {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "the rules' complete proof interval must precede the record's proof interval",
        ));
    }
    if record_proof.times.upper_utc > as_of {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            "the frozen record was not yet time-proved at this assessment cut",
        ));
    }
    let findings = record::validate(
        &loaded.manifest,
        &[],
        Some(record_proof.times),
        loaded.rules.dates,
    );
    if !findings.is_empty() {
        return Err(invalid(
            findings
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let resolved = record_sources::resolve(root, &loaded.rules, &loaded.manifest, as_of)?;
    let end = parse_utc(&format!(
        "{}-01-01T00:00:00Z",
        u32::from(resolved.outcome_year) + 1
    ))
    .ok_or_else(|| invalid("outcome year is outside the supported calendar"))?;
    if record_proof.times.upper_utc >= end {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "the record's complete external proof interval must precede the end of the scored year",
        ));
    }
    let assessment = record::scoring::paired::assess(
        &loaded.rules,
        &loaded.forecasts,
        &resolved.outcomes,
        &resolved.baselines,
    )?;
    let body = format!(
        "\"schema\":{schema},\"lane\":{lane},\"gate_eligible\":false,\"record_id\":{id},\"label\":{label},\"contact\":{contact},\"author_attestation\":{attestation},\"information_cutoff_utc\":{cut},\"as_of\":{as_of},\"outcome_year\":{year},\"record_sha256\":{record_digest},\"rules_sha256\":{rules_digest},\"rules_timestamp\":{rules_proof},\"record_timestamp\":{record_proof},\"source_availability_basis\":\"frozen_manifest_and_registered_publication_or_retrieval_bound\",\"scores\":{scores},\"evidence\":{evidence}",
        schema = js(SCHEMA),
        lane = js(if resolved.synthetic {
            "synthetic"
        } else {
            "public"
        }),
        id = js(&loaded.manifest.record_id),
        label = js(loaded.manifest.label.as_str()),
        contact = js(match loaded.manifest.contact {
            record::Contact::PreContact => "pre-contact",
            record::Contact::PostContact => "post-contact",
        }),
        attestation = js(&loaded.manifest.attestation),
        cut = js(&record::format_utc(loaded.manifest.information_cutoff_utc)),
        as_of = js(&command.as_of),
        year = resolved.outcome_year,
        record_digest = js(&loaded.record_digest.to_hex()),
        rules_digest = js(&loaded.rules_digest.to_hex()),
        rules_proof = proof_json(&rules_proof),
        record_proof = proof_json(&record_proof),
        scores = assessment.to_json(),
        evidence = resolved.evidence_json,
    );
    let digest = sha256(format!("{{{body}}}").as_bytes());
    let payload = format!("{{{body},\"digest\":{}}}", js(&digest.to_hex()));
    let markdown = format!(
        "# Frozen forecast record assessment\n\nRecord: {}. Label: {}. Evidence as of {}.\n\nRecord SHA-256: `{}`. Assessment SHA-256: `{}`.\n\nBoth RFC 3161 receipts verify against the explicitly selected trust roots; the rules precede the record.\n\n{}\n\n{}\n\n{}\n",
        markdown_text(&loaded.manifest.record_id),
        loaded.manifest.label.as_str(),
        command.as_of,
        loaded.record_digest.to_hex(),
        digest.to_hex(),
        scores_markdown(&assessment),
        resolved.evidence_markdown,
        NO_CLAIM,
    );
    Ok((payload, markdown))
}

fn markdown_text(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace(['`', '*', '_', '[', ']', '<', '>'], " ")
        .replace('|', "\\|")
}

fn rational(value: record::Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn scores_markdown(scores: &record::scoring::paired::RecordAssessment) -> String {
    use record::scoring::paired::Availability;
    let value = |v: Option<record::Rat>| v.map_or_else(|| "unavailable".into(), rational);
    let bounds = |v: fmine_numerics::exact_feas::Interval| {
        format!("[{}, {}]", rational(v.lo), rational(v.hi))
    };
    let mut out = format!(
        "Calibration status: {}. Interval-score alpha: {}. Binomial reference coverage level: {}. All fractions are exact.\n\nPaired means use the same sites with an outcome, a forecast and every registered baseline. Issuer-guidance bounds carry no inferred nominal probability. Completion bounds concern unresolved members; they are not confidence intervals. Effective sample ranges are sensitivity to the frozen correlation range, not estimated dependence.\n",
        if scores.targets.iter().any(|t| t.n_resolved > 0) {
            "descriptive"
        } else {
            "unscored"
        },
        rational(scores.alpha),
        rational(scores.coverage_level),
    );
    let _ = writeln!(out, "\n{}", record::scoring::paired::CP_ASSUMPTION);
    for target in &scores.targets {
        let _ = writeln!(
            out,
            "\n## {}\n\n{} declared sites; {} frozen live members; {} frozen not-published sites; {} resolved; {} unresolved live members; {} paired comparisons.\n\n| Method | Scored / live | Mean interval score | Paired mean |\n|---|---:|---:|---:|",
            markdown_text(&target.target),
            target.n_members,
            target.n_live,
            target.n_not_published,
            target.n_resolved,
            target.n_unresolved,
            target.n_paired,
        );
        for method in &target.methods {
            let _ = writeln!(
                out,
                "| {} | {} / {} | {} | {} |",
                markdown_text(&method.method),
                method.n_scored,
                target.n_live,
                value(method.mean_interval_score),
                value(method.paired_mean_interval_score)
            );
        }
        let _ = writeln!(
            out,
            "\nPaired mean differences: forecast minus issuer guidance {}; forecast minus persistence {}. Negative differences favor the forecast under the frozen loss, without establishing skill.\n",
            value(target.forecast_minus_issuer_guidance),
            value(target.forecast_minus_run_rate_persistence),
        );
        for method in &target.methods {
            let _ = writeln!(
                out,
                "- {}: {} hits; observed coverage {}; whole-live-cohort coverage {}; iid binomial reference interval {}; effective sample range {}.",
                markdown_text(&method.method),
                method.n_inside,
                value(method.observed_coverage),
                method
                    .whole_cohort_coverage
                    .map_or_else(|| "unavailable".into(), bounds),
                method
                    .iid_reference_interval
                    .map_or_else(|| "unavailable".into(), bounds),
                bounds(method.effective_n),
            );
        }
    }
    out.push_str("\n## Unavailable members and baselines\n\n");
    let mut missing = 0;
    for member in &scores.members {
        if let Availability::Unavailable(reason) = &member.outcome {
            let _ = writeln!(
                out,
                "- {} / {} outcome: {}.",
                markdown_text(&member.site),
                markdown_text(&member.target),
                markdown_text(reason)
            );
            missing += 1;
        }
        for baseline in &member.baselines {
            if let Availability::Unavailable(reason) = &baseline.value {
                let _ = writeln!(
                    out,
                    "- {} / {} / {}: {}.",
                    markdown_text(&member.site),
                    markdown_text(&member.target),
                    markdown_text(&baseline.baseline),
                    markdown_text(reason)
                );
                missing += 1;
            }
        }
    }
    if missing == 0 {
        out.push_str("All frozen live members and their baselines are available.\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn record_scoring_requires_explicit_record_cut_and_trust_roots() {
        let valid = args(&[
            "forecast",
            "record",
            "score",
            "--record",
            "record",
            "--as-of",
            "2030-02-01T00:00:00Z",
            "--tsa-roots",
            "trusted.pem",
        ]);
        let (command, format, _) = parse(&valid, Path::new(".")).expect("explicit inputs");
        assert_eq!(command.record, Path::new("record"));
        assert_eq!(format, OutputFormat::Json);
        for flag in ["--record", "--as-of", "--tsa-roots"] {
            let mut missing = valid.clone();
            let at = missing.iter().position(|s| s == flag).expect("flag");
            missing.drain(at..at + 2);
            assert!(parse(&missing, Path::new(".")).is_err());
        }
        for extra in [
            args(&["--trust-host-clock"]),
            args(&["--selector", "latest-as-of"]),
            args(&["--as-of", "2030-02-01T00:00:00Z"]),
            args(&["--json", "--format", "markdown"]),
        ] {
            let mut invalid = valid.clone();
            invalid.extend(extra);
            assert!(parse(&invalid, Path::new(".")).is_err());
        }
        let mut short = valid;
        short[6] = "2030-02-01".into();
        assert!(parse(&short, Path::new(".")).is_err());
    }
}
