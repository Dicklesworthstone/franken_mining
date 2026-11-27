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
//! - `runtime`: live inspection of `--obligations <path>` (`DurableObligationRegistry`, `PO-BED-5`),
//!   `--hwm <cell_id>=<seq>` (projection high-water marks checked against verified `--segment` heads),
//!   and `--little-queue <name>:<lambda>:<mu>:<occupancy>` (`PO-BED-7` Little's-law `ρ ≤ 0.7` audit),
//!   falling back honestly to `"not_deployed"` when a runtime input is not supplied;
//! - `healthy`, `findings_count`, `findings`: top-level fault list across every checked dimension.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_ledger::verify::verify_segment;
use fmine_public::sources::{load_issuer_profile, registered_issuers};
use fmine_registry::RegistryCatalog;
use fmine_runtime::obligations::DurableObligationRegistry;
use fmine_units::ExactQty;

use crate::repair::inspect_journal_path;
use crate::{Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// CLI usage summary for `fmine doctor`.
pub const USAGE: &str = "doctor [--segment <path>]... [--journal <path>]... [--obligations <path>]... [--hwm <cell_id>=<seq>]... [--little-queue <name>:<lambda>:<mu>:<occupancy>]...";

/// Machine-readable documentation block for `fmine robot-docs`.
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.doctor.v2\",",
    "\"checks\":[\"registries\",\"issuers\",\"pins\",\"budgets\",\"lifts\",\"forecasts\",\"chains\",\"runtime\",\"findings\"],",
    "\"flags\":[\"--segment <path>\",\"--journal <path>\",\"--obligations <path>\",\"--hwm <cell_id>=<seq>\",\"--little-queue <name>:<lambda>:<mu>:<occupancy>\"],",
    "\"scope\":\"Read-only diagnostics across chains, HWMs, foundation pins, LOC budgets, durable OB-* obligations (PO-BED-5), and Little's-law queues (PO-BED-7); tampers, torn tails, open obligations, lagging/ahead HWMs, and overloaded queues are reported as findings rather than silently fixed\"}"
);

/// Projection high-water mark assertion (`--hwm <cell_id>=<projected_seq>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwmTarget {
    pub cell_id: String,
    pub projected_seq: u64,
}

/// Exact Little's-law queue observation (`--little-queue <name>:<lambda>:<mu>:<occupancy>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LittleQueueSample {
    pub name: String,
    pub arrival_rate_per_s: ExactQty,
    pub service_rate_per_s: ExactQty,
    pub mean_occupancy: ExactQty,
}

/// Parsed `fmine doctor` command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Command {
    /// Optional `FMT-SEG-1` Bedrock segment files to hash-chain and MMR verify.
    pub segments: Vec<PathBuf>,
    /// Optional `FJR1` / `RLJ1` journal files to hash-chain verify.
    pub journals: Vec<PathBuf>,
    /// Optional `FMT-SEG-1` durable `OB-*` obligation journal segments to audit (`PO-BED-5`).
    pub obligations: Vec<PathBuf>,
    /// Optional projection high-water marks to verify against `--segment` authority heads.
    pub hwms: Vec<HwmTarget>,
    /// Optional ingest queue observations to audit under Little's law (`PO-BED-7`, `ρ ≤ 0.7`).
    pub little_queues: Vec<LittleQueueSample>,
}

impl Command {
    /// True when no explicit diagnostic target flags were provided.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.segments.is_empty()
            && self.journals.is_empty()
            && self.obligations.is_empty()
            && self.hwms.is_empty()
            && self.little_queues.is_empty()
    }
}

/// Parse `doctor` options after global options are stripped.
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
            "--obligations" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --obligations")?;
                cmd.obligations.push(PathBuf::from(value));
            }
            "--hwm" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --hwm")?;
                cmd.hwms.push(parse_hwm(value)?);
            }
            "--little-queue" => {
                i += 1;
                let value = words
                    .get(i)
                    .filter(|v| !v.is_empty() && !v.starts_with("--"))
                    .ok_or("missing value for --little-queue")?;
                cmd.little_queues.push(parse_little_queue(value)?);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown flag '{flag}'")),
            extra => return Err(format!("unexpected argument '{extra}' after 'doctor'")),
        }
        i += 1;
    }
    Ok(cmd)
}

fn parse_hwm(raw: &str) -> Result<HwmTarget, String> {
    let Some((cell, seq_str)) = raw.split_once('=') else {
        return Err(format!(
            "invalid --hwm '{raw}' (expected <cell_id>=<projected_seq>)"
        ));
    };
    let cell_id = cell.trim();
    if cell_id.is_empty() {
        return Err(format!("invalid --hwm '{raw}': empty cell_id"));
    }
    let projected_seq = seq_str
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("invalid --hwm '{raw}': projected_seq must be a u64"))?;
    Ok(HwmTarget {
        cell_id: cell_id.to_string(),
        projected_seq,
    })
}

fn parse_little_queue(raw: &str) -> Result<LittleQueueSample, String> {
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() != 4 {
        return Err(format!(
            "invalid --little-queue '{raw}' (expected <name>:<lambda>:<mu>:<occupancy>)"
        ));
    }
    let name = parts[0].trim();
    if name.is_empty() {
        return Err(format!("invalid --little-queue '{raw}': empty queue name"));
    }
    let arrival_rate_per_s = ExactQty::from_decimal_str(parts[1].trim())
        .map_err(|_| format!("invalid --little-queue '{raw}': lambda must be a decimal number"))?;
    if arrival_rate_per_s.signum() < 0 {
        return Err(format!(
            "invalid --little-queue '{raw}': lambda must be >= 0"
        ));
    }
    let service_rate_per_s = ExactQty::from_decimal_str(parts[2].trim())
        .map_err(|_| format!("invalid --little-queue '{raw}': mu must be a decimal number"))?;
    if service_rate_per_s.signum() <= 0 {
        return Err(format!("invalid --little-queue '{raw}': mu must be > 0"));
    }
    let mean_occupancy = ExactQty::from_decimal_str(parts[3].trim()).map_err(|_| {
        format!("invalid --little-queue '{raw}': occupancy must be a decimal number")
    })?;
    if mean_occupancy.signum() < 0 {
        return Err(format!(
            "invalid --little-queue '{raw}': occupancy must be >= 0"
        ));
    }
    Ok(LittleQueueSample {
        name: name.to_string(),
        arrival_rate_per_s,
        service_rate_per_s,
        mean_occupancy,
    })
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

    let mut findings = Vec::new();
    let issuers = diagnose_issuers(workspace_root, &mut findings);
    let pins = diagnose_pins(workspace_root, &mut findings);
    let budgets = diagnose_budgets(workspace_root, &mut findings);
    let lifts = diagnose_lifts(workspace_root, &mut findings);
    let forecasts = diagnose_forecasts(workspace_root);
    let (chains, authority_heads) = diagnose_chains(cmd, workspace_root, &mut findings);
    let runtime = diagnose_runtime(cmd, workspace_root, &authority_heads, &mut findings);

    let healthy = findings.is_empty();
    let findings_json: Vec<String> = findings
        .iter()
        .map(|f| format!("\"{}\"", escape_json(f)))
        .collect();

    let payload = format!(
        "{{\"registries_ok\":true,\"healthy\":{healthy},\"findings_count\":{},\"findings\":[{}],\"units_count\":{},\"crs_count\":{},\"refusals_count\":{},\"verbs_count\":{},\"issuers\":[{}],\"pins\":{pins},\"budgets\":{budgets},\"lifts\":{lifts},\"forecasts\":{forecasts},\"chains\":{chains},\"runtime\":{runtime}}}",
        findings.len(),
        findings_json.join(","),
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

fn diagnose_issuers(workspace_root: &Path, findings: &mut Vec<String>) -> Vec<String> {
    issuer_source_checks(workspace_root)
        .iter()
        .map(|(name, checks)| match checks {
            Ok(checks) => {
                let count = |idx: usize| checks.iter().filter(|(_, c)| c[idx]).count();
                let rows: Vec<String> = checks
                    .iter()
                    .map(|(file, [present, bytes_ok, digest_ok])| {
                        if *present && (!*bytes_ok || !*digest_ok) {
                            findings.push(format!(
                                "issuer:{name}:{file}: present corpus file failed byte/digest check"
                            ));
                        }
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
            Err(what) => {
                findings.push(format!("issuer:{name}: registry error: {what}"));
                format!(
                    "{{\"issuer\":\"{}\",\"registry_ok\":false,\"error\":\"{}\"}}",
                    escape_json(name),
                    escape_json(what)
                )
            }
        })
        .collect()
}

fn diagnose_pins(workspace_root: &Path, findings: &mut Vec<String>) -> String {
    let path = workspace_root.join("franken-stack.lock");
    let Ok(text) = std::fs::read_to_string(&path) else {
        findings.push("pins: franken-stack.lock missing or unreadable".to_string());
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
            asupersync_pin = rest
                .strip_suffix('"')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
        }
    }
    if foundations == 0 {
        findings.push("pins: franken-stack.lock contains 0 foundation entries".to_string());
    }
    if asupersync_pin.is_none() {
        findings.push("pins: missing asupersync pin in franken-stack.lock".to_string());
    }
    let pin_json = asupersync_pin.map_or_else(
        || "null".to_string(),
        |p| format!("\"{}\"", escape_json(&p)),
    );
    format!("{{\"present\":true,\"foundations\":{foundations},\"asupersync_pin\":{pin_json}}}")
}

fn diagnose_budgets(workspace_root: &Path, findings: &mut Vec<String>) -> String {
    let path = workspace_root.join("registries/loc_budget.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        findings.push("budgets: registries/loc_budget.toml missing or unreadable".to_string());
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
                 over_ceiling: &mut Vec<String>,
                 findings: &mut Vec<String>| {
        if let (Some(crate_name), Some(p), Some(c)) = (name, planned, ceiling) {
            let src = workspace_root.join("crates").join(&crate_name).join("src");
            if src.is_dir() {
                *checked += 1;
                let actual = count_rs_lines(&src);
                if actual > c {
                    over_ceiling.push(format!("\"{}\"", escape_json(&crate_name)));
                    findings.push(format!(
                        "budget:{crate_name}: {actual} LOC exceeds ceiling {c}"
                    ));
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
                findings,
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
        findings,
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

fn diagnose_lifts(workspace_root: &Path, findings: &mut Vec<String>) -> String {
    let path = workspace_root.join("registries/lifts.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        findings.push("lifts: registries/lifts.toml missing or unreadable".to_string());
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
            findings.push(format!("lifts: missing lifted target '{target}'"));
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

fn resolve_input_path(raw: &Path, workspace_root: &Path) -> PathBuf {
    if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        workspace_root.join(raw)
    }
}

fn diagnose_chains(
    cmd: &Command,
    workspace_root: &Path,
    findings: &mut Vec<String>,
) -> (String, BTreeMap<String, Result<u64, String>>) {
    let mut authority_heads: BTreeMap<String, Result<u64, String>> = BTreeMap::new();
    let seg_rows: Vec<String> = cmd
        .segments
        .iter()
        .map(|raw| {
            let path = resolve_input_path(raw, workspace_root);
            match verify_segment(&path) {
                Ok(v) => {
                    let ok = v.torn_bytes == 0;
                    if ok {
                        match authority_heads.get(&v.cell_id) {
                            None => {
                                authority_heads.insert(v.cell_id.clone(), Ok(v.record_count));
                            }
                            Some(Ok(prev)) if *prev == v.record_count => {}
                            Some(Ok(prev)) => {
                                let msg = format!(
                                    "conflicting segment record counts for {}: {} vs {}",
                                    v.cell_id, prev, v.record_count
                                );
                                findings.push(format!(
                                    "chain:segment:{}: {msg}",
                                    raw.display()
                                ));
                                authority_heads.insert(v.cell_id.clone(), Err(msg));
                            }
                            Some(Err(_)) => {
                                // Preserve existing unhealthy status for this cell_id; never
                                // let a second healthy segment mask an unhealthy one.
                            }
                        }
                    } else {
                        authority_heads
                            .insert(v.cell_id.clone(), Err(format!("segment status {}", v.status)));
                        findings.push(format!(
                            "chain:segment:{}: {} ({} torn bytes)",
                            raw.display(),
                            v.status,
                            v.torn_bytes
                        ));
                    }
                    format!(
                        "{{\"path\":\"{}\",\"kind\":\"fmt_seg_1\",\"ok\":{ok},\"status\":\"{}\",\"cell_id\":\"{}\",\"records\":{},\"head_hash\":\"{}\",\"mmr_root\":\"{}\",\"verified_bytes\":{},\"total_bytes\":{},\"torn_bytes\":{},\"issue\":{}}}",
                        escape_json(&raw.to_string_lossy()),
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
                    )
                }
                Err(err) => {
                    findings.push(format!("chain:segment:{}: corrupt: {err}", raw.display()));
                    format!(
                        "{{\"path\":\"{}\",\"kind\":\"fmt_seg_1\",\"ok\":false,\"status\":\"corrupt\",\"error\":\"{}\"}}",
                        escape_json(&raw.to_string_lossy()),
                        escape_json(&err.to_string())
                    )
                }
            }
        })
        .collect();

    let journal_rows: Vec<String> = cmd
        .journals
        .iter()
        .map(|raw| {
            let path = resolve_input_path(raw, workspace_root);
            match inspect_journal_path(&path, None) {
                Ok(j) => {
                    let ok = j.torn_bytes == 0;
                    if !ok {
                        findings.push(format!(
                            "chain:journal:{}: torn_tail ({} torn bytes)",
                            raw.display(),
                            j.torn_bytes
                        ));
                    }
                    format!(
                        "{{\"path\":\"{}\",\"kind\":\"{}\",\"ok\":{ok},\"status\":\"{}\",\"records\":{},\"head_hash\":\"{}\",\"verified_bytes\":{},\"total_bytes\":{},\"torn_bytes\":{},\"issue\":{}}}",
                        escape_json(&raw.to_string_lossy()),
                        j.kind_label,
                        if ok { "clean" } else { "torn_tail" },
                        j.retained_records,
                        j.last_verified_head,
                        j.verified_bytes,
                        j.original_bytes,
                        j.torn_bytes,
                        if ok {
                            "null".to_string()
                        } else {
                            format!("\"{}\"", escape_json(&j.reason))
                        }
                    )
                }
                Err(err) => {
                    findings.push(format!(
                        "chain:journal:{}: corrupt: {}",
                        raw.display(),
                        err.what
                    ));
                    format!(
                        "{{\"path\":\"{}\",\"kind\":\"journal\",\"ok\":false,\"status\":\"corrupt\",\"error\":\"{}\"}}",
                        escape_json(&raw.to_string_lossy()),
                        escape_json(&err.what)
                    )
                }
            }
        })
        .collect();

    (
        format!(
            "{{\"segments\":[{}],\"journals\":[{}]}}",
            seg_rows.join(","),
            journal_rows.join(",")
        ),
        authority_heads,
    )
}

fn diagnose_runtime(
    cmd: &Command,
    workspace_root: &Path,
    authority_heads: &BTreeMap<String, Result<u64, String>>,
    findings: &mut Vec<String>,
) -> String {
    let hwm_json = if cmd.hwms.is_empty() {
        "\"not_deployed\"".to_string()
    } else {
        let rows: Vec<String> = cmd
            .hwms
            .iter()
            .map(|h| match authority_heads.get(&h.cell_id) {
                None => {
                    findings.push(format!(
                        "hwm:{}: no verified --segment authority supplied for cell",
                        h.cell_id
                    ));
                    format!(
                        "{{\"cell_id\":\"{}\",\"projected_seq\":{},\"authority_records\":null,\"lag\":null,\"ok\":false,\"status\":\"unknown_authority\"}}",
                        escape_json(&h.cell_id),
                        h.projected_seq
                    )
                }
                Some(Err(reason)) => {
                    findings.push(format!(
                        "hwm:{}: authority segment unhealthy ({reason})",
                        h.cell_id
                    ));
                    format!(
                        "{{\"cell_id\":\"{}\",\"projected_seq\":{},\"authority_records\":null,\"lag\":null,\"ok\":false,\"status\":\"authority_unhealthy\",\"error\":\"{}\"}}",
                        escape_json(&h.cell_id),
                        h.projected_seq,
                        escape_json(reason)
                    )
                }
                Some(Ok(auth_records)) => {
                    if h.projected_seq > *auth_records {
                        findings.push(format!(
                            "hwm:{}: projected_seq {} exceeds authority records {}",
                            h.cell_id, h.projected_seq, auth_records
                        ));
                        format!(
                            "{{\"cell_id\":\"{}\",\"projected_seq\":{},\"authority_records\":{auth_records},\"lag\":null,\"ok\":false,\"status\":\"ahead_of_authority\"}}",
                            escape_json(&h.cell_id),
                            h.projected_seq
                        )
                    } else if h.projected_seq < *auth_records {
                        let lag = auth_records - h.projected_seq;
                        findings.push(format!(
                            "hwm:{}: projection lags authority by {lag} batch(es)",
                            h.cell_id
                        ));
                        format!(
                            "{{\"cell_id\":\"{}\",\"projected_seq\":{},\"authority_records\":{auth_records},\"lag\":{lag},\"ok\":false,\"status\":\"lagging\"}}",
                            escape_json(&h.cell_id),
                            h.projected_seq
                        )
                    } else {
                        format!(
                            "{{\"cell_id\":\"{}\",\"projected_seq\":{},\"authority_records\":{auth_records},\"lag\":0,\"ok\":true,\"status\":\"caught_up\"}}",
                            escape_json(&h.cell_id),
                            h.projected_seq
                        )
                    }
                }
            })
            .collect();
        format!("[{}]", rows.join(","))
    };

    let obligations_json = if cmd.obligations.is_empty() {
        "\"not_deployed\"".to_string()
    } else {
        let rows: Vec<String> = cmd
            .obligations
            .iter()
            .map(|raw| {
                let path = resolve_input_path(raw, workspace_root);
                match DurableObligationRegistry::open(&path) {
                    Ok(reg) => {
                        let open = reg.registry().open_obligations();
                        let quiescent = open.is_empty();
                        if !quiescent {
                            findings.push(format!(
                                "obligations:{}: {} open obligation(s) violate quiescence (PO-BED-5)",
                                raw.display(),
                                open.len()
                            ));
                        }
                        let open_rows: Vec<String> = open
                            .iter()
                            .map(|rec| {
                                format!(
                                    "{{\"id\":\"{}\",\"kind\":\"{}\",\"region\":\"{:?}\",\"opened_at_ns\":{}}}",
                                    escape_json(&rec.obligation_id),
                                    rec.kind.code(),
                                    rec.owner_region,
                                    rec.opened_at_ns
                                )
                            })
                            .collect();
                        format!(
                            "{{\"path\":\"{}\",\"ok\":{quiescent},\"status\":\"{}\",\"quiescent\":{quiescent},\"open_count\":{},\"total_events\":{},\"open_obligations\":[{}]}}",
                            escape_json(&raw.to_string_lossy()),
                            if quiescent { "quiescent" } else { "open_obligations" },
                            open.len(),
                            reg.registry().journal().len(),
                            open_rows.join(",")
                        )
                    }
                    Err(err) => {
                        findings.push(format!(
                            "obligations:{}: corrupt or unreadable: {err}",
                            raw.display()
                        ));
                        format!(
                            "{{\"path\":\"{}\",\"ok\":false,\"status\":\"corrupt\",\"quiescent\":false,\"error\":\"{}\"}}",
                            escape_json(&raw.to_string_lossy()),
                            escape_json(&err.to_string())
                        )
                    }
                }
            })
            .collect();
        format!("[{}]", rows.join(","))
    };

    let little_json = if cmd.little_queues.is_empty() {
        "\"not_deployed\"".to_string()
    } else {
        let target_rho = ExactQty::new(7, 10).unwrap_or_else(|_| ExactQty::from_integer(1));
        let unity = ExactQty::from_integer(1);
        let rows: Vec<String> = cmd
            .little_queues
            .iter()
            .map(|q| {
                let rho_res = q.arrival_rate_per_s.checked_div(q.service_rate_per_s);
                let Ok(rho) = rho_res else {
                    findings.push(format!("little_queue:{}: utilization overflow", q.name));
                    return format!(
                        "{{\"name\":\"{}\",\"ok\":false,\"status\":\"overflow\"}}",
                        escape_json(&q.name)
                    );
                };
                let (mut ok, mut status) = match (rho.checked_cmp(unity), rho.checked_cmp(target_rho))
                {
                    (Err(_), _) | (_, Err(_)) => {
                        findings.push(format!(
                            "little_queue:{}: exact utilization comparison overflowed (PO-BED-7)",
                            q.name
                        ));
                        (false, "comparison_overflow")
                    }
                    (Ok(Ordering::Greater | Ordering::Equal), _) => {
                        findings.push(format!(
                            "little_queue:{}: unstable utilization rho={:.6} >= 1.0 (PO-BED-7)",
                            q.name,
                            rho.to_f64()
                        ));
                        (false, "unstable")
                    }
                    (_, Ok(Ordering::Greater)) => {
                        findings.push(format!(
                            "little_queue:{}: utilization rho={:.6} exceeds 0.7 target (PO-BED-7)",
                            q.name,
                            rho.to_f64()
                        ));
                        (false, "over_target")
                    }
                    _ if q.arrival_rate_per_s.signum() == 0 && q.mean_occupancy.signum() > 0 => {
                        findings.push(format!(
                            "little_queue:{}: stalled backlog (L={:.6} with lambda=0)",
                            q.name,
                            q.mean_occupancy.to_f64()
                        ));
                        (false, "stalled_backlog")
                    }
                    _ => (true, "healthy"),
                };
                let implied_wait_json = if q.arrival_rate_per_s.signum() > 0 {
                    match q.mean_occupancy.checked_div(q.arrival_rate_per_s) {
                        Ok(wait) => format!("{:.6}", wait.to_f64()),
                        Err(_) => {
                            if ok {
                                ok = false;
                                status = "wait_overflow";
                            }
                            findings.push(format!(
                                "little_queue:{}: implied wait L/lambda overflowed",
                                q.name
                            ));
                            "null".to_string()
                        }
                    }
                } else {
                    "null".to_string()
                };
                format!(
                    "{{\"name\":\"{}\",\"ok\":{ok},\"status\":\"{status}\",\"lambda_per_s\":{:.6},\"mu_per_s\":{:.6},\"rho\":{:.6},\"rho_ceiling\":0.700000,\"mean_occupancy\":{:.6},\"implied_wait_s\":{implied_wait_json}}}",
                    escape_json(&q.name),
                    q.arrival_rate_per_s.to_f64(),
                    q.service_rate_per_s.to_f64(),
                    rho.to_f64(),
                    q.mean_occupancy.to_f64()
                )
            })
            .collect();
        format!("[{}]", rows.join(","))
    };

    format!(
        "{{\"daemon_status\":\"not_deployed\",\"high_water_marks\":{hwm_json},\"durable_obligations\":{obligations_json},\"little_queue_audit\":{little_json}}}"
    )
}
