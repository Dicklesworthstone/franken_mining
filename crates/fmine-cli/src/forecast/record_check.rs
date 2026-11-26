//! DESIGN (BD-067/334): check a prepared public forecast record before obtaining external time proofs.
//! Only declared input bytes are opened; no outcome is needed and no record or digest file is written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_calib::record::{self, Label, RecordManifest, RecordRules, SiteStatus, TargetMetric};
use fmine_core::hash::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::source;
use fmine_public::sources::{IssuerProfile, is_issuer_name, registry_path};
use fmine_registry::sources::parse_sources_registry;

use crate::{Envelope, NextAction, OutputFormat, json_string as js};

pub const USAGE: &str = "forecast record check --record <directory> [--format json|markdown]";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"forecast record check --record <directory>","schema":"fmine.forecast-record-check.v1","inputs":"Exact rules.toml, manifest.toml and forecasts.csv; the issuer registry and every declared input file","checks":"Rules/record binding, complete live-site/target forecast population, both canonical baseline declarations, declared timing feasibility, registry metadata and exact declared source bytes; existing rules.sha256/record.sha256 files must match when present","reads":"Only declared manifest inputs; no future outcome or baseline files are required","outputs":"Canonical FMRUL1 rules and FMREC1 record digests, source checks and declared population","scope":"Read-only preflight; no file creation, model reproduction, source-value extraction, baseline computation, signature verification, trusted time sample or issuance. A successful check does not establish that a proof deadline was met or authenticate the supplied author attestation."}"#;

const VERB: &str = "forecast record check";
const MAX_TEXT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const NO_CLAIM: &str = "Read-only checks of supplied forecast inputs and registered source bytes. The author attestation remains supplied and unverified. Model reproduction, source-value interpretation, baseline values and external timestamps are not checked. No trusted time is sampled, no deadline is established as met, and no forecast is issued or calibrated.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub record: PathBuf,
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
            "--record" | "--workspace" | "--format" => {
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
    if words != ["forecast", "record", "check"] {
        return Err(format!("expected {USAGE}"));
    }
    let format = match values.get("--format").copied() {
        None | Some("json") => OutputFormat::Json,
        Some("markdown" | "md") if !json => OutputFormat::Markdown,
        Some("markdown" | "md") => return Err("--json conflicts with Markdown format".into()),
        _ => return Err("--format requires json|markdown".into()),
    };
    Ok((
        Command {
            record: PathBuf::from(
                values
                    .get("--record")
                    .copied()
                    .ok_or_else(|| format!("{VERB} requires --record"))?,
            ),
        },
        format,
        values
            .get("--workspace")
            .map_or_else(|| default_root.to_path_buf(), PathBuf::from),
    ))
}

#[must_use]
pub fn execute(command: &Command, root: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Review the separate frozen-record timestamp and scoring requirements",
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

fn invalid(why: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        format!("forecast record check: {why}"),
    )
}

fn mismatch(why: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::DigestMismatch505, why)
}

fn checked_stored_digest(path: &Path, digest: Digest32) -> Result<bool, Refusal> {
    let present = path.try_exists().map_err(|error| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {error}", path.display()),
        )
    })?;
    if present {
        let (stored, _) = source::read(path, VERB, 1024)?;
        if record::parse_hex32(stored.trim_end_matches('\n')) != Some(digest.0) {
            return Err(mismatch(format!(
                "{} differs from the exact supplied record bytes",
                path.display()
            )));
        }
    }
    Ok(present)
}

fn check_sources(
    root: &Path,
    profile: &IssuerProfile,
    manifest: &RecordManifest,
    rules: &RecordRules,
) -> Result<Vec<String>, Refusal> {
    let sources = fmine_public::forecast::manifest::bind(profile, rules, manifest)?;
    manifest.inputs.iter().zip(sources).map(|(input, source)| {
        let registered = source.ok_or_else(|| invalid(format!("{} is not in the issuer's source registry", input.doc_id)))?;
        let (actual, anchor) = source::read_bytes(&root.join(registered.dir).join(&input.doc_id), VERB, MAX_SOURCE_BYTES)?;
        if actual.len() as u64 != input.bytes || anchor.artifact_sha256 != input.sha256 {
            return Err(mismatch(format!("{} bytes differ from the frozen input manifest", input.doc_id)));
        }
        Ok(format!(
            "{{\"doc_id\":{},\"url\":{},\"bytes\":{},\"sha256\":{},\"bytes_verified\":true,\"declared_retrieved_utc\":{},\"availability_basis\":\"frozen_manifest_retrieval_declaration\",\"registered_publication_date\":{},\"publication_basis\":{}}}",
            js(&input.doc_id), js(&input.url), input.bytes, js(&Digest32(input.sha256).to_hex()), js(&record::format_utc(input.retrieved_utc)),
            registered.published_at.map_or_else(|| "null".into(), js),
            registered.published_at_basis.map_or_else(|| js("manifest_declared_unverified"), js),
        ))
    }).collect()
}

fn perform(command: &Command, root: &Path) -> Result<(String, String), Refusal> {
    let dir = root.join(&command.record);
    let (rules_text, _) = source::read(&dir.join("rules.toml"), VERB, MAX_TEXT_BYTES)?;
    let (manifest_text, _) = source::read(&dir.join("manifest.toml"), VERB, MAX_TEXT_BYTES)?;
    let (csv, _) = source::read_bytes(&dir.join("forecasts.csv"), VERB, MAX_TEXT_BYTES)?;
    if rules_text.lines().any(record::has_open_marker) {
        return Err(invalid("rules contain an unresolved OPEN marker"));
    }
    let rules = record::parse_rules(&rules_text).map_err(invalid)?;
    record::scoring::paired::validate_rules(&rules)?;
    let manifest = record::parse_manifest(&manifest_text).map_err(invalid)?;
    let forecasts = record::parse_forecasts(&csv, &rules).map_err(invalid)?;
    let rules_digest = record::rules_digest(rules_text.as_bytes());
    let record_digest = record::record_digest(manifest_text.as_bytes(), &csv);
    if rules.record_id != manifest.record_id || rules_digest.0 != manifest.rules_sha256 {
        return Err(mismatch(
            "manifest does not bind these exact rules and record id",
        ));
    }
    for target in &rules.targets {
        if TargetMetric::from_target(target).is_none() {
            return Err(invalid(format!(
                "{} has no supported public target interpretation",
                target.id
            )));
        }
    }
    if !is_issuer_name(&rules.issuer) {
        return Err(invalid("issuer is not a source registry name"));
    }
    let (registry_text, registry_anchor) =
        source::read(&registry_path(root, &rules.issuer), VERB, MAX_TEXT_BYTES)?;
    let registry = parse_sources_registry(&registry_text).map_err(invalid)?;
    if registry.issuer != rules.issuer {
        return Err(invalid(
            "source registry issuer differs from the frozen rules",
        ));
    }
    let outcome = rules.outcome_quarter().map_err(invalid)?;
    let end = outcome
        .end_utc()
        .ok_or_else(|| invalid("outcome quarter is outside UTC"))?;
    let cut = manifest.information_cutoff_utc;
    let post_quarter = rules.is_post_quarter_nowcast();
    if post_quarter && (cut < end || manifest.label != Label::Nowcast) {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "post-quarter nowcast inputs need a post-quarter information cut and the nowcast label",
        ));
    }
    // DESIGN: check whether any whole-second proof interval could satisfy the frozen declarations.
    // Actual receipt accuracy and rule-before-record ordering remain the verifier's responsibility.
    let mut first = cut
        .checked_add(1)
        .ok_or_else(|| invalid("information cut overflows UTC"))?;
    let mut last = if post_quarter {
        rules.dates.proof_by
    } else {
        rules.dates.proof_by.min(end - 1)
    };
    match manifest.label {
        Label::Forecast => last = last.min(rules.dates.nowcast_from - 1),
        Label::Nowcast => first = first.max(rules.dates.nowcast_from),
    }
    if post_quarter {
        for source in fmine_public::forecast::nowcast_publications(&registry, outcome)? {
            last = last.min(source.day_start_utc - 1);
        }
    }
    if first > last {
        return Err(Refusal::new(
            RefusalCode::LateRegistration206,
            "declared information cut, label and outcome horizon leave no possible prospective time proof",
        ));
    }
    let profile = IssuerProfile::from_registry(registry).map_err(invalid)?;
    let inputs = check_sources(root, &profile, &manifest, &rules)?;
    let stored_rules = checked_stored_digest(&dir.join("rules.sha256"), rules_digest)?;
    let stored_record = checked_stored_digest(&dir.join("record.sha256"), record_digest)?;
    let excluded = rules
        .sites
        .iter()
        .filter(|s| s.status == SiteStatus::NotPublished)
        .map(|s| js(&s.name))
        .collect::<Vec<_>>();
    let payload = format!(
        "{{\"schema\":\"fmine.forecast-record-check.v1\",\"status\":\"inputs_valid\",\"lane\":{},\"gate_eligible\":false,\"record_id\":{},\"label\":{},\"author_role\":{},\"author_attestation\":{},\"information_cutoff_utc\":{},\"proof_by_utc\":{},\"outcome_quarter\":{},\"rules_sha256\":{},\"record_sha256\":{},\"source_registry_sha256\":{},\"stored_rules_digest_checked\":{},\"stored_record_digest_checked\":{},\"n_forecasts\":{},\"n_live_sites\":{},\"n_targets\":{},\"not_published_sites\":[{}],\"inputs\":[{}],\"model_reproduced\":false,\"baseline_values_checked\":false,\"outcomes_read\":false,\"timestamp_verification\":\"not_performed\",\"issued\":false}}",
        js(if rules.issuer == "synthetic" {
            "synthetic"
        } else {
            "public"
        }),
        js(&manifest.record_id),
        js(manifest.label.as_str()),
        js(&manifest.author_role),
        js(&manifest.attestation),
        js(&record::format_utc(cut)),
        js(&record::format_utc(rules.dates.proof_by)),
        js(&outcome.to_string()),
        js(&rules_digest.to_hex()),
        js(&record_digest.to_hex()),
        js(&Digest32(registry_anchor.artifact_sha256).to_hex()),
        stored_rules,
        stored_record,
        forecasts.len(),
        rules.sites.len() - excluded.len(),
        rules.targets.len(),
        excluded.join(","),
        inputs.join(","),
    );
    let markdown = format!(
        "# Forecast record input check\n\nInput checks passed for {} forecast rows, {} targets and {} declared source files. No future outcome files were required.\n\nRules SHA-256 (FMRUL1): `{}`.\n\nRecord SHA-256 (FMREC1): `{}`.\n\nDeclared information cut: {}. Frozen proof deadline: {}. Outcome quarter: {}.\n\n{}\n",
        forecasts.len(),
        rules.targets.len(),
        inputs.len(),
        rules_digest.to_hex(),
        record_digest.to_hex(),
        record::format_utc(cut),
        record::format_utc(rules.dates.proof_by),
        outcome,
        NO_CLAIM,
    );
    Ok((payload, markdown))
}
