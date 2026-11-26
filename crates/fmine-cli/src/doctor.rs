//! Upgraded `fmine doctor` (`BD-117`, plan §26.11, §28.4.2, §30.2).
//!
//! Preserves all existing top-level diagnostic fields (`registries_ok`, `units_count`, `crs_count`,
//! `refusals_count`, `verbs_count`, `issuers`) while adding:
//! - `pins`: `franken-stack.lock` foundation pin summary;
//! - `budgets`: `registries/loc_budget.toml` per-crate 80%-warning and overrun diagnostics;
//! - `lifts`: `registries/lifts.toml` lifted-file presence & provenance check;
//! - `forecasts`: `forecasts/` manifest/rules/timestamp presence audit;
//! - `chains`: optional `--segment <path>` (`FMT-SEG-1`) and `--journal <path>` (`FJR1`/`RLJ1`)
//!   hash-chain verification;
//! - `runtime`: honest `"not_deployed"` entries for daemon-only checks (HWMs, obligations, Little's-law queues).

use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::verify::verify_segment;
use fmine_public::sources::{load_issuer_profile, registered_issuers};
use fmine_registry::RegistryCatalog;

use crate::repair::inspect_journal_path;
use crate::{Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// CLI usage summary for `fmine doctor`.
pub const USAGE: &str = "doctor [--segment <path>]... [--journal <path>]...";

/// Machine-readable documentation block for `fmine robot-docs`.
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.doctor.v2\",",
    "\"checks\":[\"registries\",\"issuers\",\"pins\",\"budgets\",\"lifts\",\"forecasts\",\"chains\",\"runtime\"],",
    "\"flags\":[\"--segment <path>\",\"--journal <path>\"],",
    "\"scope\":\"Read-only diagnostics; tampers or torn tails are reported as findings rather than silently fixed; daemon HWMs/obligations/Little queues report not_deployed when fmined is absent\"}"
);

/// Parsed `fmine doctor` command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Command {
    /// Optional `FMT-SEG-1` Bedrock segment files to hash-chain and MMR verify.
    pub segments: Vec<PathBuf>,
    /// Optional `FJR1` / `RLJ1` journal files to hash-chain verify.
    pub journals: Vec<PathBuf>,
}

/// Parse `doctor [--segment <path>]... [--journal <path>]...` after global options are stripped.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("doctor") {
        return Err(format!("expected 'fmine {USAGE}'"));
    }
    let mut cmd = Command::default();
    let mut i = 1;
    while let Some(arg) = words.get(i) {
        match arg.as_str() {
            "--segment" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --segment")?;
                cmd.segments.push(PathBuf::from(value));
            }
            "--journal" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --journal")?;
                cmd.journals.push(PathBuf::from(value));
            }
            flag if flag.starts_with("--") => return Err(format!("unknown flag '{flag}'")),
            extra => return Err(format!("unexpected argument '{extra}' after 'doctor'")),
        }
        i += 1;
    }
    Ok(cmd)
}

/// Execute `fmine doctor`.
pub fn execute(cmd: &Command, workspace_root: &Path) -> Envelope {
    let cat = match RegistryCatalog::load_from_root(workspace_root) {
        Ok(cat) => cat,
        Err(err) => {
            return Envelope::refused(
                "doctor",
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, err.to_string()),
                vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
            );
        }
    };

    let issuers = diagnose_issuers(workspace_root);
    let pins = diagnose_pins(workspace_root);
    let budgets = diagnose_budgets(workspace_root);
    let lifts = diagnose_lifts(workspace_root);
    let forecasts = diagnose_forecasts(workspace_root);
    let chains = diagnose_chains(cmd, workspace_root);
    let runtime = concat!(
        "{\"daemon_status\":\"not_deployed\",",
        "\"high_water_marks\":\"not_deployed\",",
        "\"durable_obligations\":\"not_deployed\",",
        "\"little_queue_audit\":\"not_deployed\"}"
    );

    let payload = format!(
        "{{\"registries_ok\":true,\"units_count\":{},\"crs_count\":{},\"refusals_count\":{},\"verbs_count\":{},\"issuers\":[{}],\"pins\":{pins},\"budgets\":{budgets},\"lifts\":{lifts},\"forecasts\":{forecasts},\"chains\":{chains},\"runtime\":{runtime}}}",
        cat.unit_symbols.len(),
        cat.crs_codes.len(),
        cat.refusal_codes.len(),
        cat.verb_names.len(),
        issuers.join(",")
    );

    Envelope::ok(
        "doctor",
        payload,
        vec![NextAction::new(
            "fmine assure public --synthetic",
            "known-answer run on synthetic workbooks that exercises every ledger state",
        )],
    )
}

/// Each registered source of one issuer, as `(file, [present, bytes_ok, digest_ok])`, or why the issuer's
/// source registry does not load.
pub(crate) type SourceChecks = Result<Vec<(String, [bool; 3])>, String>;

/// Every registered issuer's sources, read from its corpus directory. Bare `fmine` triage reads the same checks.
pub(crate) fn issuer_source_checks(workspace_root: &Path) -> Vec<(String, SourceChecks)> {
    registered_issuers(workspace_root)
        .into_iter()
        .map(|name| {
            let checks = load_issuer_profile(workspace_root, &name)
                .map(|p| {
                    let dir = workspace_root.join(&p.corpus_dir);
                    p.sources
                        .iter()
                        .map(|s| {
                            let check = match std::fs::read(dir.join(&s.file_name)) {
                                Ok(bytes) => [
                                    true,
                                    bytes.len() as u64 == s.bytes,
                                    sha256(&bytes).to_hex() == s.sha256_hex,
                                ],
                                Err(_) => [false; 3],
                            };
                            (s.file_name.clone(), check)
                        })
                        .collect()
                })
                .map_err(|r| r.what);
            (name, checks)
        })
        .collect()
}

fn diagnose_issuers(workspace_root: &Path) -> Vec<String> {
    issuer_source_checks(workspace_root)
        .iter()
        .map(|(name, checks)| match checks {
            Ok(checks) => {
                let count = |idx: usize| checks.iter().filter(|(_, c)| c[idx]).count();
                let rows: Vec<String> = checks
                    .iter()
                    .map(|(file, [present, bytes_ok, digest_ok])| {
                        format!(
                            "{{\"file\":\"{}\",\"present\":{present},\"bytes_ok\":{bytes_ok},\"digest_ok\":{digest_ok}}}",
                            escape_json(file)
                        )
                    })
                    .collect();
                format!(
                    "{{\"issuer\":\"{}\",\"registry_ok\":true,\"sources_registered\":{},\"sources_present\":{},\"sources_verified\":{},\"sources\":[{}]}}",
                    escape_json(name),
                    checks.len(),
                    count(0),
                    count(2),
                    rows.join(",")
                )
            }
            Err(what) => format!(
                "{{\"issuer\":\"{}\",\"registry_ok\":false,\"error\":\"{}\"}}",
                escape_json(name),
                escape_json(what)
            ),
        })
        .collect()
}

fn diagnose_pins(workspace_root: &Path) -> String {
    let path = workspace_root.join("franken-stack.lock");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "{\"present\":false,\"foundations\":0,\"asupersync_pin\":null}".to_string();
    };
    let mut foundations = 0usize;
    let mut asupersync_pin: Option<String> = None;
    let mut in_asupersync = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line == "[[foundation]]" {
            foundations += 1;
            in_asupersync = false;
        } else if line == "name = \"asupersync\"" {
            in_asupersync = true;
        } else if in_asupersync && let Some(rest) = line.strip_prefix("pin = \"") {
            asupersync_pin = rest.strip_suffix('"').map(ToOwned::to_owned);
        }
    }
    let pin_json = asupersync_pin.map_or_else(
        || "null".to_string(),
        |p| format!("\"{}\"", escape_json(&p)),
    );
    format!("{{\"present\":true,\"foundations\":{foundations},\"asupersync_pin\":{pin_json}}}")
}

fn diagnose_budgets(workspace_root: &Path) -> String {
    let path = workspace_root.join("registries/loc_budget.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "{\"present\":false,\"crates_checked\":0,\"warn_80_pct\":[],\"over_planned\":[],\"over_ceiling\":[]}"
            .to_string();
    };
    let mut current_name: Option<String> = None;
    let mut planned: Option<u64> = None;
    let mut ceiling: Option<u64> = None;
    let mut checked = 0usize;
    let mut warn_80 = Vec::new();
    let mut over_planned = Vec::new();
    let mut over_ceiling = Vec::new();

    let flush = |name: Option<String>,
                 planned: Option<u64>,
                 ceiling: Option<u64>,
                 checked: &mut usize,
                 warn_80: &mut Vec<String>,
                 over_planned: &mut Vec<String>,
                 over_ceiling: &mut Vec<String>| {
        if let (Some(crate_name), Some(p), Some(c)) = (name, planned, ceiling) {
            let src = workspace_root.join("crates").join(&crate_name).join("src");
            if src.is_dir() {
                *checked += 1;
                let actual = count_rs_lines(&src);
                if actual > c {
                    over_ceiling.push(format!("\"{}\"", escape_json(&crate_name)));
                } else if actual > p {
                    over_planned.push(format!("\"{}\"", escape_json(&crate_name)));
                } else if actual.saturating_mul(100) >= p.saturating_mul(80) {
                    warn_80.push(format!("\"{}\"", escape_json(&crate_name)));
                }
            }
        }
    };

    for raw in text.lines() {
        let line = raw.trim();
        if line == "[[crate]]" {
            flush(
                current_name.take(),
                planned.take(),
                ceiling.take(),
                &mut checked,
                &mut warn_80,
                &mut over_planned,
                &mut over_ceiling,
            );
        } else if let Some(rest) = line.strip_prefix("name = \"") {
            current_name = rest.strip_suffix('"').map(ToOwned::to_owned);
        } else if let Some(rest) = line.strip_prefix("planned_loc = ") {
            planned = rest.trim().parse::<u64>().ok();
        } else if let Some(rest) = line.strip_prefix("ceiling_loc = ") {
            ceiling = rest.trim().parse::<u64>().ok();
        }
    }
    flush(
        current_name,
        planned,
        ceiling,
        &mut checked,
        &mut warn_80,
        &mut over_planned,
        &mut over_ceiling,
    );

    format!(
        "{{\"present\":true,\"crates_checked\":{checked},\"warn_80_pct\":[{}],\"over_planned\":[{}],\"over_ceiling\":[{}]}}",
        warn_80.join(","),
        over_planned.join(","),
        over_ceiling.join(",")
    )
}

fn count_rs_lines(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total = total.saturating_add(count_rs_lines(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            total = total.saturating_add(text.lines().count() as u64);
        }
    }
    total
}

fn diagnose_lifts(workspace_root: &Path) -> String {
    let path = workspace_root.join("registries/lifts.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "{\"present\":false,\"entries\":0,\"missing_targets\":[]}".to_string();
    };
    let mut entries = 0usize;
    let mut missing = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line == "[[lift]]" {
            entries += 1;
        } else if let Some(rest) = line.strip_prefix("target = \"")
            && let Some(target) = rest.strip_suffix('"')
            && !workspace_root.join(target).is_file()
        {
            missing.push(format!("\"{}\"", escape_json(target)));
        }
    }
    format!(
        "{{\"present\":true,\"entries\":{entries},\"missing_targets\":[{}]}}",
        missing.join(",")
    )
}

pub(crate) fn diagnose_forecasts(workspace_root: &Path) -> String {
    let dir = workspace_root.join("forecasts");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return "{\"present\":false,\"rules_files\":0,\"tsr_proofs\":0,\"records\":0}".to_string();
    };
    let mut rules_files = 0usize;
    let mut tsr_proofs = 0usize;
    let mut records = 0usize;
    let mut stack = vec![dir];
    while let Some(cur) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&cur) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".rules.toml") || name == "rules.toml" {
                    rules_files += 1;
                } else if name.ends_with(".tsr") {
                    tsr_proofs += 1;
                } else if name == "record.toml" || name == "forecasts.csv" {
                    records += 1;
                }
            }
        }
    }
    let _ = entries;
    format!(
        "{{\"present\":true,\"rules_files\":{rules_files},\"tsr_proofs\":{tsr_proofs},\"records\":{records}}}"
    )
}

fn diagnose_chains(cmd: &Command, workspace_root: &Path) -> String {
    let seg_rows: Vec<String> = cmd
        .segments
        .iter()
        .map(|raw| {
            let path = if raw.is_absolute() {
                raw.clone()
            } else {
                workspace_root.join(raw)
            };
            match verify_segment(&path) {
                Ok(v) => format!(
                    "{{\"path\":\"{}\",\"kind\":\"fmt_seg_1\",\"ok\":{},\"status\":\"{}\",\"cell_id\":\"{}\",\"records\":{},\"head_hash\":\"{}\",\"mmr_root\":\"{}\",\"verified_bytes\":{},\"total_bytes\":{},\"torn_bytes\":{},\"issue\":{}}}",
                    escape_json(&raw.to_string_lossy()),
                    v.torn_bytes == 0,
                    v.status,
                    escape_json(&v.cell_id),
                    v.record_count,
                    v.head_hash,
                    v.mmr_root,
                    v.verified_bytes,
                    v.total_bytes,
                    v.torn_bytes,
                    v.issue.as_ref().map_or_else(
                        || "null".to_string(),
                        |s| format!("\"{}\"", escape_json(s))
                    )
                ),
                Err(err) => format!(
                    "{{\"path\":\"{}\",\"kind\":\"fmt_seg_1\",\"ok\":false,\"status\":\"corrupt\",\"error\":\"{}\"}}",
                    escape_json(&raw.to_string_lossy()),
                    escape_json(&err.to_string())
                ),
            }
        })
        .collect();

    let journal_rows: Vec<String> = cmd
        .journals
        .iter()
        .map(|raw| {
            let path = if raw.is_absolute() {
                raw.clone()
            } else {
                workspace_root.join(raw)
            };
            match inspect_journal_path(&path, None) {
                Ok(j) => format!(
                    "{{\"path\":\"{}\",\"kind\":\"{}\",\"ok\":{},\"status\":\"{}\",\"records\":{},\"head_hash\":\"{}\",\"verified_bytes\":{},\"total_bytes\":{},\"torn_bytes\":{},\"issue\":{}}}",
                    escape_json(&raw.to_string_lossy()),
                    j.kind_label,
                    j.torn_bytes == 0,
                    if j.torn_bytes == 0 { "clean" } else { "torn_tail" },
                    j.retained_records,
                    j.last_verified_head,
                    j.verified_bytes,
                    j.original_bytes,
                    j.torn_bytes,
                    if j.torn_bytes == 0 {
                        "null".to_string()
                    } else {
                        format!("\"{}\"", escape_json(&j.reason))
                    }
                ),
                Err(err) => format!(
                    "{{\"path\":\"{}\",\"kind\":\"journal\",\"ok\":false,\"status\":\"corrupt\",\"error\":\"{}\"}}",
                    escape_json(&raw.to_string_lossy()),
                    escape_json(&err.what)
                ),
            }
        })
        .collect();

    format!(
        "{{\"segments\":[{}],\"journals\":[{}]}}",
        seg_rows.join(","),
        journal_rows.join(",")
    )
}
