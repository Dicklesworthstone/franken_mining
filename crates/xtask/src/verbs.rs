//! The CLI verb registry (`registries/verbs.toml`), its generated catalogue, and drift check
//! (`BD-113`, plan §30.2).
//!
//! - `gen-verbs` writes [`CATALOG_PATH`] from [`REGISTRY_PATH`]. The file is committed; nothing is
//!   generated at build time (L-2).
//! - `check-verbs` (also run by `check-all`) validates the registry schema, regenerates the
//!   catalogue in memory, and requires the committed file to match byte for byte.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use fmine_core::hash::sha256;

/// The verb registry, relative to the workspace root.
pub const REGISTRY_PATH: &str = "registries/verbs.toml";
/// The generated verb catalogue, relative to the workspace root.
pub const CATALOG_PATH: &str = "crates/fmine-cli/src/verb_catalog.rs";

const FIELDS: [&str; 4] = ["path", "subsystem", "mutates_authority", "description"];

/// The 18 named subsystems of plan §0.5.
const SUBSYSTEMS: [&str; 18] = [
    "Bedrock",
    "Plumb",
    "Lodestar",
    "Corebox",
    "Strata",
    "Highwall",
    "Pushback",
    "Headframe",
    "Crucible",
    "Freeboard",
    "Muster",
    "Touchstone",
    "Bullion",
    "Hallmark",
    "Helm",
    "Portal",
    "Adit",
    "Testpit",
];

/// One validated `[[verb]]` row from `registries/verbs.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbRow {
    /// 1-based line of `[[verb]]`.
    pub line: usize,
    /// Space-separated canonical CLI verb path (e.g. `"robot schema"`).
    pub path: String,
    /// Named §0.5 subsystem owning this verb.
    pub subsystem: String,
    /// Whether executing this verb mutates authority state.
    pub mutates_authority: bool,
    /// Concise human/robot description of the verb's contract.
    pub description: String,
}

/// Summary report emitted by `check-verbs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbReport {
    pub rows: usize,
    pub mutating: usize,
    pub read_only: usize,
}

impl VerbReport {
    /// One deterministic JSON log line.
    #[must_use]
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"check-verbs\",\"rows\":{},\"mutating\":{},\"read_only\":{}}}",
            self.rows, self.mutating, self.read_only
        )
    }
}

fn only_comment(tail: &str) -> Result<(), String> {
    let t = tail.trim_start();
    if t.is_empty() || t.starts_with('#') {
        Ok(())
    } else {
        Err(format!("unexpected trailing text {t:?}"))
    }
}

fn parse_basic_string(raw: &str) -> Result<String, String> {
    let Some(body) = raw.strip_prefix('"') else {
        return Err(format!("expected basic quoted string, got {raw:?}"));
    };
    if raw.starts_with("\"\"\"") {
        return Err("multi-line strings are outside the verbs registry schema".to_string());
    }
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                only_comment(&body[i + 1..])?;
                return Ok(out);
            }
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => {
                    return Err(format!(
                        "escape \\{other} is outside the verbs registry schema (only \\\" and \\\\)"
                    ));
                }
                None => return Err("unterminated string".to_string()),
            },
            c if (c as u32) < 0x20 => {
                return Err(format!("control character U+{:04X} in string", c as u32));
            }
            c => out.push(c),
        }
    }
    Err("unterminated string".to_string())
}

fn parse_bool(raw: &str) -> Result<bool, String> {
    let (token, tail) = match raw.find(|c: char| c.is_whitespace() || c == '#') {
        Some(i) => (&raw[..i], &raw[i..]),
        None => (raw, ""),
    };
    let val = match token {
        "true" => true,
        "false" => false,
        other => return Err(format!("expected boolean true|false, got {other:?}")),
    };
    only_comment(tail)?;
    Ok(val)
}

fn is_valid_verb_path(path: &str) -> bool {
    !path.is_empty()
        && path.trim() == path
        && !path.contains("  ")
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b' ' || b == b'-')
        && path
            .split(' ')
            .all(|seg| !seg.is_empty() && !seg.starts_with('-') && !seg.ends_with('-'))
}

#[derive(Default)]
struct PendingRow {
    line: usize,
    seen: BTreeSet<String>,
    path: Option<String>,
    subsystem: Option<String>,
    mutates_authority: Option<bool>,
    description: Option<String>,
}

impl PendingRow {
    fn finish(self, seen_paths: &mut BTreeSet<String>) -> Result<VerbRow, String> {
        let line = self.line;
        let path = self
            .path
            .ok_or_else(|| format!("line {line}: [[verb]] lacks path"))?;
        if !is_valid_verb_path(&path) {
            return Err(format!(
                "line {line}: invalid verb path {path:?} (expected lowercase space-separated segments)"
            ));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(format!("line {line}: duplicate verb path {path:?}"));
        }
        let subsystem = self
            .subsystem
            .ok_or_else(|| format!("line {line}: [[verb]] lacks subsystem"))?;
        if !SUBSYSTEMS.contains(&subsystem.as_str()) {
            return Err(format!(
                "line {line}: unknown subsystem {subsystem:?} (must be one of plan §0.5's 18)"
            ));
        }
        let mutates_authority = self
            .mutates_authority
            .ok_or_else(|| format!("line {line}: [[verb]] lacks mutates_authority"))?;
        let description = self
            .description
            .ok_or_else(|| format!("line {line}: [[verb]] lacks description"))?;
        if description.is_empty() || description.trim() != description {
            return Err(format!(
                "line {line}: description is empty or has surrounding whitespace"
            ));
        }
        Ok(VerbRow {
            line,
            path,
            subsystem,
            mutates_authority,
            description,
        })
    }
}

/// Parse `registries/verbs.toml` (schema version 1), refusing any schema violation with line number.
pub fn parse_registry(text: &str) -> Result<Vec<VerbRow>, String> {
    let mut saw_schema_version = false;
    let mut rows = Vec::new();
    let mut seen_paths = BTreeSet::new();
    let mut pending: Option<PendingRow> = None;

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if let Some(rest) = s.strip_prefix("[[") {
            let Some((table, tail)) = rest.split_once("]]") else {
                return Err(format!("line {line}: unterminated `[[` header"));
            };
            only_comment(tail).map_err(|e| format!("line {line}: {e}"))?;
            let table = table.trim();
            if table != "verb" {
                return Err(format!("line {line}: unknown table [[{table}]]"));
            }
            if !saw_schema_version {
                return Err(format!(
                    "line {line}: schema_version = 1 must appear before the first [[verb]]"
                ));
            }
            if let Some(prev) = pending.take() {
                rows.push(prev.finish(&mut seen_paths)?);
            }
            pending = Some(PendingRow {
                line,
                ..PendingRow::default()
            });
            continue;
        }
        if s.starts_with('[') {
            return Err(format!(
                "line {line}: plain [table] headers are outside the schema; use [[verb]]"
            ));
        }
        let Some((k, v)) = s.split_once('=') else {
            return Err(format!("line {line}: expected `key = value`"));
        };
        let key = k.trim();
        let val = v.trim();
        match pending.as_mut() {
            None => {
                if key != "schema_version" {
                    return Err(format!(
                        "line {line}: unexpected top-level key {key:?}; only schema_version = 1 is allowed"
                    ));
                }
                if saw_schema_version {
                    return Err(format!("line {line}: duplicate schema_version"));
                }
                let (token, tail) = match val.find(|c: char| c.is_whitespace() || c == '#') {
                    Some(i) => (&val[..i], &val[i..]),
                    None => (val, ""),
                };
                only_comment(tail).map_err(|e| format!("line {line}: {e}"))?;
                if token != "1" {
                    return Err(format!(
                        "line {line}: schema_version must be 1, got {token:?}"
                    ));
                }
                saw_schema_version = true;
            }
            Some(row) => {
                if !FIELDS.contains(&key) {
                    return Err(format!("line {line}: unknown [[verb]] field {key:?}"));
                }
                if !row.seen.insert(key.to_string()) {
                    return Err(format!("line {line}: duplicate field {key:?} in [[verb]]"));
                }
                match key {
                    "path" => {
                        row.path = Some(
                            parse_basic_string(val)
                                .map_err(|e| format!("line {line}: path: {e}"))?,
                        );
                    }
                    "subsystem" => {
                        row.subsystem = Some(
                            parse_basic_string(val)
                                .map_err(|e| format!("line {line}: subsystem: {e}"))?,
                        );
                    }
                    "mutates_authority" => {
                        row.mutates_authority = Some(
                            parse_bool(val)
                                .map_err(|e| format!("line {line}: mutates_authority: {e}"))?,
                        );
                    }
                    "description" => {
                        row.description = Some(
                            parse_basic_string(val)
                                .map_err(|e| format!("line {line}: description: {e}"))?,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(last) = pending.take() {
        rows.push(last.finish(&mut seen_paths)?);
    }
    if !saw_schema_version {
        return Err("missing top-level schema_version = 1".to_string());
    }
    if rows.is_empty() {
        return Err(format!("{REGISTRY_PATH} has no [[verb]] row"));
    }
    Ok(rows)
}

/// Deterministically render the compiled Rust catalogue from parsed `rows` and registry digest.
#[must_use]
pub fn render_catalog(rows: &[VerbRow], registry_sha256: &str) -> String {
    let n = rows.len();
    let mut o = String::new();
    o.push_str(
        "//! The compiled CLI verb catalogue (`BD-113`, plan §30.2), generated from `registries/verbs.toml`.\n",
    );
    o.push_str("//!\n");
    o.push_str(
        "//! @generated by `cargo run -p xtask -- gen-verbs`; do not edit. `check-verbs` fails when this\n",
    );
    o.push_str("//! file differs from what the registry generates.\n");
    let _ = writeln!(o, "//!\n//! Registry sha256: {registry_sha256}\n");
    o.push_str("/// One registered CLI verb row from `registries/verbs.toml`.\n");
    o.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    o.push_str("pub struct VerbSpec {\n");
    o.push_str("    pub path: &'static str,\n");
    o.push_str("    pub subsystem: &'static str,\n");
    o.push_str("    pub mutates_authority: bool,\n");
    o.push_str("    pub description: &'static str,\n");
    o.push_str("}\n\n");
    o.push_str("/// Every registered CLI verb, in registry order.\n");
    let _ = writeln!(o, "pub const VERB_CATALOG: [VerbSpec; {n}] = [");
    for r in rows {
        let _ = write!(
            o,
            "    VerbSpec {{\n        path: {:?},\n        subsystem: {:?},\n        mutates_authority: {},\n        description: {:?},\n    }},\n",
            r.path, r.subsystem, r.mutates_authority, r.description
        );
    }
    o.push_str("];\n\n");
    o.push_str(
        "/// Look up a registered CLI verb specification by its canonical space-separated path.\n",
    );
    o.push_str("#[must_use]\n");
    o.push_str("pub fn lookup_verb(path: &str) -> Option<&'static VerbSpec> {\n");
    o.push_str("    VERB_CATALOG.iter().find(|spec| spec.path == path)\n");
    o.push_str("}\n");
    o
}

/// Validate `registry` bytes and require `catalog` to match the generated output byte for byte.
pub fn check_texts(registry: &[u8], catalog: &str) -> Result<VerbReport, Vec<String>> {
    let text = std::str::from_utf8(registry)
        .map_err(|e| vec![format!("{REGISTRY_PATH} is not UTF-8: {e}")])?;
    let rows = parse_registry(text).map_err(|e| vec![format!("{REGISTRY_PATH}: {e}")])?;
    let expected = render_catalog(&rows, &sha256(registry).to_hex());
    if catalog != expected {
        return Err(vec![format!(
            "{CATALOG_PATH} differs from what {REGISTRY_PATH} generates; run `cargo run -p xtask -- gen-verbs` and commit both"
        )]);
    }
    let mutating = rows.iter().filter(|r| r.mutates_authority).count();
    Ok(VerbReport {
        rows: rows.len(),
        mutating,
        read_only: rows.len().saturating_sub(mutating),
    })
}

/// `check-verbs` on the workspace at `root`.
pub fn check_verbs(root: &Path) -> Result<VerbReport, Vec<String>> {
    let registry = fs::read(root.join(REGISTRY_PATH))
        .map_err(|e| vec![format!("cannot read {REGISTRY_PATH}: {e}")])?;
    let catalog = fs::read_to_string(root.join(CATALOG_PATH))
        .map_err(|e| vec![format!("cannot read {CATALOG_PATH}: {e}")])?;
    check_texts(&registry, &catalog)
}

/// `gen-verbs`: writes [`CATALOG_PATH`] when its contents differ; returns whether it wrote.
pub fn gen_verbs(root: &Path) -> Result<bool, Vec<String>> {
    let registry = fs::read(root.join(REGISTRY_PATH))
        .map_err(|e| vec![format!("cannot read {REGISTRY_PATH}: {e}")])?;
    let text = std::str::from_utf8(&registry)
        .map_err(|e| vec![format!("{REGISTRY_PATH} is not UTF-8: {e}")])?;
    let rows = parse_registry(text).map_err(|e| vec![format!("{REGISTRY_PATH}: {e}")])?;
    let rendered = render_catalog(&rows, &sha256(&registry).to_hex());
    let path = root.join(CATALOG_PATH);
    if fs::read_to_string(&path).is_ok_and(|t| t == rendered) {
        return Ok(false);
    }
    fs::write(&path, rendered).map_err(|e| vec![format!("cannot write {CATALOG_PATH}: {e}")])?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = r#"# Fixture verbs registry
schema_version = 1

[[verb]]
path = "doctor"
subsystem = "Adit"
mutates_authority = false
description = "Verify \"health\" & chains"

[[verb]]
path = "repair apply"
subsystem = "Adit"
mutates_authority = true
description = "Apply a verified repair plan by digest"
"#;

    #[test]
    fn test_verbs_generator_golden_output() {
        let rows = parse_registry(FIXTURE).expect("fixture parses");
        let rendered = render_catalog(&rows, "deadbeef");
        let expected = r#"//! The compiled CLI verb catalogue (`BD-113`, plan §30.2), generated from `registries/verbs.toml`.
//!
//! @generated by `cargo run -p xtask -- gen-verbs`; do not edit. `check-verbs` fails when this
//! file differs from what the registry generates.
//!
//! Registry sha256: deadbeef

/// One registered CLI verb row from `registries/verbs.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerbSpec {
    pub path: &'static str,
    pub subsystem: &'static str,
    pub mutates_authority: bool,
    pub description: &'static str,
}

/// Every registered CLI verb, in registry order.
pub const VERB_CATALOG: [VerbSpec; 2] = [
    VerbSpec {
        path: "doctor",
        subsystem: "Adit",
        mutates_authority: false,
        description: "Verify \"health\" & chains",
    },
    VerbSpec {
        path: "repair apply",
        subsystem: "Adit",
        mutates_authority: true,
        description: "Apply a verified repair plan by digest",
    },
];

/// Look up a registered CLI verb specification by its canonical space-separated path.
#[must_use]
pub fn lookup_verb(path: &str) -> Option<&'static VerbSpec> {
    VERB_CATALOG.iter().find(|spec| spec.path == path)
}
"#;
        assert_eq!(rendered, expected);
        let report = check_texts(
            FIXTURE.as_bytes(),
            &render_catalog(&rows, &sha256(FIXTURE.as_bytes()).to_hex()),
        )
        .expect("matching catalog passes");
        assert_eq!(
            report,
            VerbReport {
                rows: 2,
                mutating: 1,
                read_only: 1,
            }
        );
    }

    #[test]
    fn test_committed_verbs_registry_and_catalog_agree() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let report =
            check_verbs(&root).expect("committed verbs catalog matches registries/verbs.toml");
        assert!(
            report.rows >= 78,
            "expected >= 78 verbs, got {}",
            report.rows
        );
        assert_eq!(report.mutating + report.read_only, report.rows);
    }

    #[test]
    fn test_stale_or_invalid_verbs_registry_refused() {
        let rows = parse_registry(FIXTURE).expect("fixture parses");
        let valid_cat = render_catalog(&rows, &sha256(FIXTURE.as_bytes()).to_hex());
        let drifted = valid_cat.replace("repair apply", "repair apply ");
        let err = check_texts(FIXTURE.as_bytes(), &drifted).unwrap_err();
        assert!(err[0].contains("differs from what"), "{err:?}");

        let dup = format!(
            "{FIXTURE}\n[[verb]]\npath = \"doctor\"\nsubsystem = \"Adit\"\nmutates_authority = false\ndescription = \"dup\"\n"
        );
        assert!(
            parse_registry(&dup)
                .unwrap_err()
                .contains("duplicate verb path")
        );

        let bad_sub = FIXTURE.replace("subsystem = \"Adit\"", "subsystem = \"Bogus\"");
        assert!(
            parse_registry(&bad_sub)
                .unwrap_err()
                .contains("unknown subsystem")
        );

        let bad_bool = FIXTURE.replace("mutates_authority = true", "mutates_authority = \"yes\"");
        assert!(
            parse_registry(&bad_bool)
                .unwrap_err()
                .contains("expected boolean")
        );

        let bad_ver = FIXTURE.replace("schema_version = 1", "schema_version = 2");
        assert!(
            parse_registry(&bad_ver)
                .unwrap_err()
                .contains("schema_version must be 1")
        );
    }
}
