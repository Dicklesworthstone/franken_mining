//! The refusal registry, its generated catalogue and the drift check (BD-328.S2, bd-h0q1.2; plan §8.4, §30.1).
//!
//! - `gen-refusals` writes [`CATALOG_PATH`] from [`REGISTRY_PATH`]. The file is committed; nothing is
//!   generated at build time (L-2).
//! - `check-refusals` (also run by `check-all`) regenerates the catalogue in memory and requires the committed
//!   file byte for byte, then applies four rules:
//!   - (a) every active row's `"<nnn> <plan_text>"` is a whole item of its family's §8.4 row, and every §8.4
//!     code has an active row;
//!   - (b) every exit code follows §30.1's family rule or is one of [`EXIT_EXCEPTIONS`];
//!   - (c) every `FMINE-REF-` token with digits in `crates/**/*.rs` and `crates/*/CONTRACT.md` is a registry
//!     code;
//!   - (d) no fix says to clamp or re-normalize ("refuse, never clamp").
//!
//! A proposed row (a code awaiting the owner's plan amendment) is a warning, never an error.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use fmine_core::hash::sha256;
use fmine_registry::toml_subset::{TomlRow, TomlValue, parse_subset};

/// The registry, relative to the workspace root.
pub const REGISTRY_PATH: &str = "registries/refusals.toml";
/// The generated catalogue, relative to the workspace root.
pub const CATALOG_PATH: &str = "crates/fmine-evidence/src/refusal_catalog.rs";
/// The plan whose §8.4 the registry must match.
pub const PLAN_PATH: &str = "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md";

const PLAN_SECTION: &str = "### 8.4 Refusal codes";
const FIELDS: [&str; 8] = [
    "code",
    "family",
    "key",
    "exit_code",
    "status",
    "plan_text",
    "what",
    "fix",
];
const CLAMP_WORDS: [&str; 5] = [
    "clamp",
    "re-normalize",
    "renormalize",
    "re-normalise",
    "renormalise",
];

/// The listed exceptions to §30.1's family rule: `(code number, exit code, reason)`.
pub const EXIT_EXCEPTIONS: [(u16, u8, &str); 2] = [
    (
        504,
        7,
        "a safety refusal numbered in the 5xx family (§8.4, §16.3)",
    ),
    (
        506,
        5,
        "§30.1: a missing or unreadable registered input names the file and is never a digest-mismatch \
         code; exit 5 is foundation I/O",
    ),
];

/// A row's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// A §8.4 code.
    Active,
    /// Awaiting the owner's plan amendment; a warning in the check.
    Proposed,
    /// Held, not yet used.
    Reserved,
}

impl Status {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "proposed" => Some(Self::Proposed),
            "reserved" => Some(Self::Reserved),
            _ => None,
        }
    }

    fn variant(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Proposed => "Proposed",
            Self::Reserved => "Reserved",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Proposed => "proposed",
            Self::Reserved => "reserved",
        }
    }
}

/// One `[[refusal]]` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalRow {
    /// 1-based line of `[[refusal]]`.
    pub line: usize,
    /// The three digits, e.g. 101.
    pub number: u16,
    pub key: String,
    pub exit_code: u8,
    pub status: Status,
    pub plan_text: String,
    pub what: String,
    pub fix: String,
}

impl RefusalRow {
    /// `FMINE-REF-<number>`.
    #[must_use]
    pub fn code(&self) -> String {
        format!("FMINE-REF-{}", self.number)
    }

    /// The enum variant: the key in CamelCase, then the number (`support_mismatch`, 102 → `SupportMismatch102`).
    #[must_use]
    pub fn variant(&self) -> String {
        let mut out = String::new();
        for part in self.key.split('_') {
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                out.push_str(chars.as_str());
            }
        }
        let _ = write!(out, "{}", self.number);
        out
    }

    fn spec_const(&self) -> String {
        format!("SPEC_{}_{}", self.number, self.key.to_ascii_uppercase())
    }

    fn family(&self) -> u16 {
        self.number / 100
    }
}

fn is_key(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && !s.ends_with('_')
        && !s.contains("__")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn row_str<'a>(row: &'a TomlRow, key: &str) -> Result<&'a str, String> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) if !s.is_empty() && s.trim() == s => Ok(s),
        Some(TomlValue::Str(_)) => Err(format!(
            "line {}: {key} is empty or has surrounding spaces",
            row.line
        )),
        Some(TomlValue::Int(_)) => Err(format!("line {}: {key} must be a string", row.line)),
        None => Err(format!("line {}: [[refusal]] lacks {key}", row.line)),
    }
}

/// Parses registry v2, refusing the first row that breaks the schema. Rows must ascend by code.
pub fn parse_registry(text: &str) -> Result<Vec<RefusalRow>, String> {
    let doc = parse_subset(text).map_err(|e| format!("{REGISTRY_PATH}: {e}"))?;
    match doc.top.as_slice() {
        [f] if f.key == "schema_version" && f.value == TomlValue::Int(2) => {}
        [f] if f.key == "schema_version" => {
            return Err(format!("line {}: schema_version must be 2", f.line));
        }
        _ => {
            return Err(
                "the only top-level key must be schema_version = 2, before the first row"
                    .to_string(),
            );
        }
    }
    let mut rows: Vec<RefusalRow> = Vec::new();
    let mut keys = BTreeSet::new();
    for row in &doc.rows {
        if row.table != "refusal" {
            return Err(format!(
                "line {}: unknown table [[{}]]",
                row.line, row.table
            ));
        }
        if let Some(extra) = row
            .fields
            .iter()
            .find(|f| !FIELDS.contains(&f.key.as_str()))
        {
            return Err(format!("line {}: unknown field {}", extra.line, extra.key));
        }
        let code = row_str(row, "code")?;
        let digits = code
            .strip_prefix("FMINE-REF-")
            .filter(|d| d.len() == 3 && d.bytes().all(|b| b.is_ascii_digit()))
            .filter(|d| matches!(d.as_bytes().first(), Some(b'1'..=b'5')))
            .ok_or_else(|| {
                format!(
                    "line {}: code {code:?} is not FMINE-REF-nnn with nnn in 100..=599",
                    row.line
                )
            })?;
        let number: u16 = digits
            .parse()
            .map_err(|_| format!("line {}: code {code:?} is not a number", row.line))?;
        let family = row_str(row, "family")?;
        if family != format!("{}xx", number / 100) {
            return Err(format!(
                "line {}: {code} has family {family:?}, not \"{}xx\"",
                row.line,
                number / 100
            ));
        }
        let key = row_str(row, "key")?;
        if !is_key(key) {
            return Err(format!("line {}: key {key:?} is not snake_case", row.line));
        }
        if !keys.insert(key.to_string()) {
            return Err(format!("line {}: key {key:?} is used twice", row.line));
        }
        let exit_code = match row.get("exit_code").map(|f| &f.value) {
            Some(TomlValue::Int(n)) if (3..=8).contains(n) => {
                u8::try_from(*n).map_err(|_| format!("line {}: exit_code {n}", row.line))?
            }
            Some(TomlValue::Int(n)) => {
                return Err(format!(
                    "line {}: exit_code {n} is outside 3..=8 (§30.1)",
                    row.line
                ));
            }
            Some(TomlValue::Str(_)) => {
                return Err(format!("line {}: exit_code must be an integer", row.line));
            }
            None => return Err(format!("line {}: [[refusal]] lacks exit_code", row.line)),
        };
        let status_word = row_str(row, "status")?;
        let status = Status::parse(status_word).ok_or_else(|| {
            format!(
                "line {}: status {status_word:?} is not active, proposed or reserved",
                row.line
            )
        })?;
        if let Some(prev) = rows.last()
            && prev.number >= number
        {
            return Err(format!(
                "line {}: {code} follows FMINE-REF-{}; rows ascend by code",
                row.line, prev.number
            ));
        }
        rows.push(RefusalRow {
            line: row.line,
            number,
            key: key.to_string(),
            exit_code,
            status,
            plan_text: row_str(row, "plan_text")?.to_string(),
            what: row_str(row, "what")?.to_string(),
            fix: row_str(row, "fix")?.to_string(),
        });
    }
    if rows.is_empty() {
        return Err(format!("{REGISTRY_PATH} has no [[refusal]] row"));
    }
    Ok(rows)
}

/// The catalogue source for `rows`; `registry_sha256` is the hex digest of the registry's bytes.
#[must_use]
pub fn render_catalog(rows: &[RefusalRow], registry_sha256: &str) -> String {
    let n = rows.len();
    let mut o = String::new();
    o.push_str(
        "//! The compiled refusal catalogue (BD-328, plan §30.1), generated from \
         `registries/refusals.toml`.\n",
    );
    o.push_str("//!\n");
    o.push_str(
        "//! @generated by `cargo run -p xtask -- gen-refusals`; do not edit. `check-refusals` fails \
         when this\n",
    );
    o.push_str("//! file differs from what the registry generates.\n");
    let _ = writeln!(o, "//!\n//! Registry sha256: {registry_sha256}\n");
    o.push_str("use crate::{RefusalSpec, RefusalStatus};\n\n");
    o.push_str(
        "/// A registered refusal code. The enum is closed: a code the registry lacks cannot be named.\n",
    );
    o.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\n");
    o.push_str("pub enum RefusalCode {\n");
    for r in rows {
        let _ = writeln!(
            o,
            "    /// `{}` ({}): {}.",
            r.code(),
            r.status.word(),
            r.plan_text
        );
        let _ = writeln!(o, "    {},", r.variant());
    }
    o.push_str("}\n\n");
    o.push_str("impl RefusalCode {\n");
    o.push_str("    /// Every code, in registry order.\n");
    let _ = writeln!(o, "    pub const ALL: [Self; {n}] = [");
    for r in rows {
        let _ = writeln!(o, "        Self::{},", r.variant());
    }
    o.push_str("    ];\n\n");
    o.push_str("    /// This code's registry row.\n");
    o.push_str("    #[must_use]\n");
    o.push_str("    pub fn spec(self) -> &'static RefusalSpec {\n");
    o.push_str("        match self {\n");
    for r in rows {
        let _ = writeln!(
            o,
            "            Self::{} => &{},",
            r.variant(),
            r.spec_const()
        );
    }
    o.push_str("        }\n");
    o.push_str("    }\n");
    o.push_str("}\n\n");
    o.push_str("/// Every registry row, in registry order.\n");
    let _ = writeln!(o, "pub const CATALOG: [RefusalSpec; {n}] = [");
    for r in rows {
        let _ = writeln!(o, "    {},", r.spec_const());
    }
    o.push_str("];\n");
    for r in rows {
        let _ = write!(
            o,
            "\nconst {}: RefusalSpec = RefusalSpec {{\n    code: {:?},\n    key: {:?},\n    \
             exit_code: {},\n    status: RefusalStatus::{},\n    summary: {:?},\n    fix: {:?},\n}};\n",
            r.spec_const(),
            r.code(),
            r.key,
            r.exit_code,
            r.status.variant(),
            r.what,
            r.fix
        );
    }
    o
}

/// §8.4's table cells by family digit (1..=5), from the plan text.
pub fn plan_families(plan: &str) -> Result<BTreeMap<u16, String>, String> {
    let start = plan
        .lines()
        .position(|l| l.trim_end() == PLAN_SECTION)
        .ok_or_else(|| format!("{PLAN_PATH} has no {PLAN_SECTION:?} heading"))?;
    let mut out = BTreeMap::new();
    for line in plan.lines().skip(start + 1) {
        if line.starts_with('#') {
            break;
        }
        let Some(rest) = line.strip_prefix("| `FMINE-REF-") else {
            continue;
        };
        let family = rest
            .get(..1)
            .and_then(|d| d.parse::<u16>().ok())
            .filter(|d| (1..=5).contains(d) && rest.get(1..4) == Some("xx`"))
            .ok_or_else(|| format!("§8.4 row {line:?} does not start | `FMINE-REF-<d>xx`"))?;
        let cell = line
            .split('|')
            .nth(2)
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .ok_or_else(|| format!("§8.4 {family}xx row has no codes cell"))?;
        if out.insert(family, cell.to_string()).is_some() {
            return Err(format!("§8.4 has two {family}xx rows"));
        }
    }
    if out.is_empty() {
        return Err(format!("{PLAN_SECTION} has no FMINE-REF rows"));
    }
    Ok(out)
}

/// The items of one §8.4 cell: `(number, byte offset, item text)`. An item starts the cell or follows "; ",
/// with three digits and a space.
#[must_use]
pub fn plan_items(cell: &str) -> Vec<(u16, usize, &str)> {
    let starts: Vec<(u16, usize)> = cell
        .char_indices()
        .filter(|&(i, _)| i == 0 || cell.get(..i).is_some_and(|p| p.ends_with("; ")))
        .filter_map(|(i, _)| {
            let head = cell.get(i..i + 4)?;
            let (digits, space) = head.split_at(3);
            (space == " " && digits.bytes().all(|b| b.is_ascii_digit()))
                .then(|| digits.parse().ok().map(|n| (n, i)))
                .flatten()
        })
        .collect();
    starts
        .iter()
        .enumerate()
        .map(|(k, &(n, i))| {
            let end = starts.get(k + 1).map_or(cell.len(), |&(_, j)| j - 2);
            (n, i, cell.get(i..end).unwrap_or(""))
        })
        .collect()
}

/// Every `FMINE-REF-<digits>` token of `text`: `(1-based line, digits)`. A family wildcard (`1xx`) is not a
/// token; a typo such as a letter O among the digits is, and so fails the check.
#[must_use]
pub fn code_tokens(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(pos) = rest.find("FMINE-REF-") {
            let tail = rest.get(pos + "FMINE-REF-".len()..).unwrap_or("");
            let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
            let wildcard = tail.get(digits.len()..).is_some_and(|t| t.starts_with('x'));
            if !digits.is_empty() && !wildcard {
                out.push((idx + 1, digits));
            }
            rest = tail;
        }
    }
    out
}

/// What one run of the check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalReport {
    pub rows: usize,
    pub active: usize,
    pub proposed: usize,
    pub reserved: usize,
    pub plan_codes: usize,
    pub files: usize,
    pub tokens: usize,
    /// One line per proposed row.
    pub warnings: Vec<String>,
}

impl RefusalReport {
    /// One JSON log line.
    #[must_use]
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"check-refusals\",\"rows\":{},\"active\":{},\"proposed\":{},\"reserved\":{},\
             \"plan_codes\":{},\"files\":{},\"tokens\":{},\"warnings\":{}}}",
            self.rows,
            self.active,
            self.proposed,
            self.reserved,
            self.plan_codes,
            self.files,
            self.tokens,
            self.warnings.len()
        )
    }
}

/// The check on given texts: the registry's bytes, the committed catalogue, the plan, and every
/// `(path, text)` whose tokens rule (c) checks.
pub fn check_texts(
    registry: &[u8],
    catalog: &str,
    plan: &str,
    sources: &[(String, String)],
) -> Result<RefusalReport, Vec<String>> {
    let text = std::str::from_utf8(registry)
        .map_err(|e| vec![format!("{REGISTRY_PATH} is not UTF-8: {e}")])?;
    let rows = parse_registry(text).map_err(|e| vec![format!("{REGISTRY_PATH} {e}")])?;
    let mut errors = Vec::new();

    if catalog != render_catalog(&rows, &sha256(registry).to_hex()) {
        errors.push(format!(
            "{CATALOG_PATH} differs from what {REGISTRY_PATH} generates; run `cargo run -p xtask -- \
             gen-refusals` and commit both"
        ));
    }

    // (a) plan §8.4, both directions.
    let families = match plan_families(plan) {
        Ok(f) => f,
        Err(e) => {
            errors.push(e);
            return Err(errors);
        }
    };
    let mut plan_codes = BTreeSet::new();
    for (&family, cell) in &families {
        for (n, _, item) in plan_items(cell) {
            plan_codes.insert(n);
            if n / 100 != family {
                errors.push(format!("§8.4 {family}xx row lists {n}"));
            }
            match rows.iter().find(|r| r.number == n) {
                None => errors.push(format!("§8.4 code {n} ({item:?}) has no registry row")),
                Some(r) if r.status != Status::Active => errors.push(format!(
                    "FMINE-REF-{n} (line {}) is {} but §8.4 lists it; mark it active",
                    r.line,
                    r.status.word()
                )),
                Some(_) => {}
            }
        }
    }
    for r in rows.iter().filter(|r| r.status == Status::Active) {
        let needle = format!("{} {}", r.number, r.plan_text);
        let item = families.get(&r.family()).and_then(|cell| {
            plan_items(cell)
                .into_iter()
                .find(|&(n, _, _)| n == r.number)
        });
        match item {
            None => errors.push(format!(
                "{} (line {}) is active but not in §8.4",
                r.code(),
                r.line
            )),
            Some((_, _, item)) if item != needle => errors.push(format!(
                "{} (line {}): \"{needle}\" is not §8.4's item {item:?}",
                r.code(),
                r.line
            )),
            Some(_) => {}
        }
    }

    // (b) exit codes, §30.1.
    for r in &rows {
        let allowed: &[u8] = match r.family() {
            1 | 2 => &[3],
            3 | 4 => &[7],
            _ => &[6, 8],
        };
        let excepted = EXIT_EXCEPTIONS
            .iter()
            .any(|&(n, e, _)| n == r.number && e == r.exit_code);
        if !allowed.contains(&r.exit_code) && !excepted {
            errors.push(format!(
                "{} (line {}): exit_code {} breaks §30.1 ({}xx -> {allowed:?}) and is not a listed \
                 exception",
                r.code(),
                r.line,
                r.exit_code,
                r.family()
            ));
        }
    }
    for &(n, e, _) in &EXIT_EXCEPTIONS {
        if !rows.iter().any(|r| r.number == n && r.exit_code == e) {
            errors.push(format!(
                "listed exit exception FMINE-REF-{n} -> {e} matches no registry row"
            ));
        }
    }

    // (c) every token in the crates is a registry code.
    let registered: BTreeSet<String> = rows.iter().map(|r| r.number.to_string()).collect();
    let mut tokens = 0;
    for (path, text) in sources {
        for (line, digits) in code_tokens(text) {
            tokens += 1;
            if !registered.contains(&digits) {
                errors.push(format!(
                    "{path}:{line}: FMINE-REF-{digits} is not a registry code"
                ));
            }
        }
    }

    // (d) refuse, never clamp.
    for r in &rows {
        let fix = r.fix.to_lowercase();
        if let Some(word) = CLAMP_WORDS.iter().find(|w| fix.contains(**w)) {
            errors.push(format!(
                "{} (line {}): fix says {word:?}; a refusal never suggests clamping",
                r.code(),
                r.line
            ));
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    let count = |s: Status| rows.iter().filter(|r| r.status == s).count();
    Ok(RefusalReport {
        rows: rows.len(),
        active: count(Status::Active),
        proposed: count(Status::Proposed),
        reserved: count(Status::Reserved),
        plan_codes: plan_codes.len(),
        files: sources.len(),
        tokens,
        warnings: rows
            .iter()
            .filter(|r| r.status == Status::Proposed)
            .map(|r| {
                format!(
                    "{} (line {}) is proposed, not yet in plan §8.4 (docs/adr/ADR-REFUSALS-01.md)",
                    r.code(),
                    r.line
                )
            })
            .collect(),
    })
}

/// `crates/**/*.rs` and `crates/*/CONTRACT.md`, as `(path relative to root, text)`, in path order.
pub fn token_sources(root: &Path) -> Result<Vec<(String, String)>, Vec<String>> {
    let crates = root.join("crates");
    let mut paths = crate::collect_rs_files(&crates);
    let dirs = fs::read_dir(&crates)
        .map_err(|e| vec![format!("cannot read {}: {e}", crates.display())])?;
    for entry in dirs.flatten() {
        let contract = entry.path().join("CONTRACT.md");
        if contract.is_file() {
            paths.push(contract);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for p in paths {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .into_owned();
        match fs::read_to_string(&p) {
            Ok(text) => out.push((rel, text)),
            Err(e) => errors.push(format!("cannot read {rel}: {e}")),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// `check-refusals` on the workspace at `root`.
pub fn check_refusals(root: &Path) -> Result<RefusalReport, Vec<String>> {
    let read =
        |rel: &str| fs::read(root.join(rel)).map_err(|e| vec![format!("cannot read {rel}: {e}")]);
    let registry = read(REGISTRY_PATH)?;
    let catalog = String::from_utf8(read(CATALOG_PATH)?)
        .map_err(|e| vec![format!("{CATALOG_PATH} is not UTF-8: {e}")])?;
    let plan = String::from_utf8(read(PLAN_PATH)?)
        .map_err(|e| vec![format!("{PLAN_PATH} is not UTF-8: {e}")])?;
    check_texts(&registry, &catalog, &plan, &token_sources(root)?)
}

/// `gen-refusals`: writes the catalogue when it differs; returns whether it wrote.
pub fn gen_refusals(root: &Path) -> Result<bool, Vec<String>> {
    let registry = fs::read(root.join(REGISTRY_PATH))
        .map_err(|e| vec![format!("cannot read {REGISTRY_PATH}: {e}")])?;
    let text = std::str::from_utf8(&registry)
        .map_err(|e| vec![format!("{REGISTRY_PATH} is not UTF-8: {e}")])?;
    let rows = parse_registry(text).map_err(|e| vec![format!("{REGISTRY_PATH} {e}")])?;
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

    const FIXTURE: &str = r#"schema_version = 2

[[refusal]]
code = "FMINE-REF-101"
family = "1xx"
key = "conservation"
exit_code = 3
status = "active"
plan_text = "conservation"
what = "Conservation violated"
fix = "Open a reconciliation case."

[[refusal]]
code = "FMINE-REF-201"
family = "2xx"
key = "no_claim_in_domain"
exit_code = 3
status = "active"
plan_text = "no claim in domain"
what = "No claim covers the domain"
fix = "Produce the claim first."

[[refusal]]
code = "FMINE-REF-504"
family = "5xx"
key = "uncured_backfill"
exit_code = 7
status = "active"
plan_text = "uncured backfill (§16.3; G-HEAD-7)"
what = "Uncured \"fill\""
fix = "Wait for strength."

[[refusal]]
code = "FMINE-REF-506"
family = "5xx"
key = "input_missing_or_unreadable"
exit_code = 5
status = "proposed"
plan_text = "registered input missing or unreadable"
what = "Registered input missing"
fix = "Put the file back."
"#;

    const PLAN: &str = "# Plan\n\n### 8.4 Refusal codes\n\n| Family | Codes |\n|---|---|\n\
        | `FMINE-REF-1xx` domain-law refusals | 101 conservation |\n\
        | `FMINE-REF-2xx` evidence refusals | 201 no claim in domain |\n\
        | `FMINE-REF-5xx` integration refusals | 504 uncured backfill (§16.3; G-HEAD-7) |\n\
        \n### 8.5 Next\n| `FMINE-REF-3xx` outside the section | 301 MNPI wall |\n";

    fn rows() -> Vec<RefusalRow> {
        parse_registry(FIXTURE).expect("fixture parses")
    }

    fn catalog_for(registry: &str) -> String {
        let rows = parse_registry(registry).expect("parses");
        render_catalog(&rows, &sha256(registry.as_bytes()).to_hex())
    }

    fn check(
        registry: &str,
        plan: &str,
        sources: &[(&str, &str)],
    ) -> Result<RefusalReport, Vec<String>> {
        let sources: Vec<(String, String)> = sources
            .iter()
            .map(|(p, t)| ((*p).to_string(), (*t).to_string()))
            .collect();
        check_texts(registry.as_bytes(), &catalog_for(registry), plan, &sources)
    }

    fn token(n: u32) -> String {
        format!("FMINE-REF-{n}")
    }

    #[test]
    fn test_generator_golden_for_fixture_registry() {
        let rendered = render_catalog(&rows(), "ab12");
        let expected = r#"//! The compiled refusal catalogue (BD-328, plan §30.1), generated from `registries/refusals.toml`.
//!
//! @generated by `cargo run -p xtask -- gen-refusals`; do not edit. `check-refusals` fails when this
//! file differs from what the registry generates.
//!
//! Registry sha256: ab12

use crate::{RefusalSpec, RefusalStatus};

/// A registered refusal code. The enum is closed: a code the registry lacks cannot be named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RefusalCode {
    /// `FMINE-REF-101` (active): conservation.
    Conservation101,
    /// `FMINE-REF-201` (active): no claim in domain.
    NoClaimInDomain201,
    /// `FMINE-REF-504` (active): uncured backfill (§16.3; G-HEAD-7).
    UncuredBackfill504,
    /// `FMINE-REF-506` (proposed): registered input missing or unreadable.
    InputMissingOrUnreadable506,
}

impl RefusalCode {
    /// Every code, in registry order.
    pub const ALL: [Self; 4] = [
        Self::Conservation101,
        Self::NoClaimInDomain201,
        Self::UncuredBackfill504,
        Self::InputMissingOrUnreadable506,
    ];

    /// This code's registry row.
    #[must_use]
    pub fn spec(self) -> &'static RefusalSpec {
        match self {
            Self::Conservation101 => &SPEC_101_CONSERVATION,
            Self::NoClaimInDomain201 => &SPEC_201_NO_CLAIM_IN_DOMAIN,
            Self::UncuredBackfill504 => &SPEC_504_UNCURED_BACKFILL,
            Self::InputMissingOrUnreadable506 => &SPEC_506_INPUT_MISSING_OR_UNREADABLE,
        }
    }
}

/// Every registry row, in registry order.
pub const CATALOG: [RefusalSpec; 4] = [
    SPEC_101_CONSERVATION,
    SPEC_201_NO_CLAIM_IN_DOMAIN,
    SPEC_504_UNCURED_BACKFILL,
    SPEC_506_INPUT_MISSING_OR_UNREADABLE,
];

const SPEC_101_CONSERVATION: RefusalSpec = RefusalSpec {
    code: "FMINE-REF-101",
    key: "conservation",
    exit_code: 3,
    status: RefusalStatus::Active,
    summary: "Conservation violated",
    fix: "Open a reconciliation case.",
};

const SPEC_201_NO_CLAIM_IN_DOMAIN: RefusalSpec = RefusalSpec {
    code: "FMINE-REF-201",
    key: "no_claim_in_domain",
    exit_code: 3,
    status: RefusalStatus::Active,
    summary: "No claim covers the domain",
    fix: "Produce the claim first.",
};

const SPEC_504_UNCURED_BACKFILL: RefusalSpec = RefusalSpec {
    code: "FMINE-REF-504",
    key: "uncured_backfill",
    exit_code: 7,
    status: RefusalStatus::Active,
    summary: "Uncured \"fill\"",
    fix: "Wait for strength.",
};

const SPEC_506_INPUT_MISSING_OR_UNREADABLE: RefusalSpec = RefusalSpec {
    code: "FMINE-REF-506",
    key: "input_missing_or_unreadable",
    exit_code: 5,
    status: RefusalStatus::Proposed,
    summary: "Registered input missing",
    fix: "Put the file back.",
};
"#;
        assert_eq!(rendered, expected);
        // Deterministic: a second render is byte-identical.
        assert_eq!(render_catalog(&rows(), "ab12"), rendered);
    }

    #[test]
    fn test_registry_schema_refusals() {
        let cases: [(&str, &str, &str); 9] = [
            (
                "schema_version = 2",
                "schema_version = 1",
                "schema_version must be 2",
            ),
            (
                "key = \"conservation\"\n",
                "key = \"conservation\"\nextra = \"x\"\n",
                "unknown field extra",
            ),
            // Built with concat! so this file holds no unregistered token for rule (c).
            (
                "code = \"FMINE-REF-201\"",
                concat!("code = \"FMINE-REF-", "2O1\""),
                "is not FMINE-REF-nnn",
            ),
            ("family = \"2xx\"", "family = \"3xx\"", "has family \"3xx\""),
            (
                "key = \"no_claim_in_domain\"",
                "key = \"conservation\"",
                "is used twice",
            ),
            (
                "key = \"no_claim_in_domain\"",
                "key = \"NoClaim\"",
                "is not snake_case",
            ),
            (
                "status = \"proposed\"",
                "status = \"approved\"",
                "is not active, proposed or reserved",
            ),
            ("exit_code = 5", "exit_code = 1", "outside 3..=8"),
            ("what = \"No claim covers the domain\"\n", "", "lacks what"),
        ];
        for (from, to, want) in cases {
            let bad = FIXTURE.replacen(from, to, 1);
            assert_ne!(bad, FIXTURE, "{from}");
            let err = parse_registry(&bad).expect_err(want);
            assert!(err.contains(want), "{want}: {err}");
        }
        // Rows must ascend by code.
        let swapped = FIXTURE
            .replacen("FMINE-REF-504", &token(199), 1)
            .replacen("\"5xx\"", "\"1xx\"", 1);
        let err = parse_registry(&swapped).expect_err("order");
        assert!(err.contains("rows ascend by code"), "{err}");
    }

    #[test]
    fn test_plan_items_keep_semicolons_inside_an_item() {
        let cell = "107 undeclared unit; 108 assay lookahead (`AssayLagLookahead`; §6.3, G-TIME-2)";
        let items = plan_items(cell);
        assert_eq!(
            items,
            vec![
                (107, 0, "107 undeclared unit"),
                (
                    108,
                    21,
                    "108 assay lookahead (`AssayLagLookahead`; §6.3, G-TIME-2)"
                ),
            ]
        );
    }

    #[test]
    fn test_fixture_passes_and_warns_on_proposed() {
        let report = check(
            FIXTURE,
            PLAN,
            &[("crates/a/src/lib.rs", "// FMINE-REF-101 and FMINE-REF-1xx")],
        )
        .expect("passes");
        assert_eq!((report.rows, report.active, report.proposed), (4, 3, 1));
        assert_eq!((report.plan_codes, report.files, report.tokens), (3, 1, 1));
        assert_eq!(report.warnings.len(), 1);
        assert!(
            report.warnings[0].starts_with("FMINE-REF-506 (line"),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn test_rule_a_plan_agreement_both_directions() {
        // A shortened plan_text is not the whole item.
        let short = FIXTURE.replacen(
            "plan_text = \"uncured backfill (§16.3; G-HEAD-7)\"",
            "plan_text = \"uncured backfill\"",
            1,
        );
        let errs = check(&short, PLAN, &[]).expect_err("short");
        assert!(
            errs.iter()
                .any(|e| e.contains("\"504 uncured backfill\" is not §8.4's item")),
            "{errs:?}"
        );
        // A plan code without a row.
        let plan = PLAN.replacen(
            "201 no claim in domain |",
            "201 no claim in domain; 202 not observable |",
            1,
        );
        let errs = check(FIXTURE, &plan, &[]).expect_err("missing row");
        assert!(errs.iter().any(|e| e.contains("§8.4 code 202")), "{errs:?}");
        // A proposed row the plan already lists.
        let plan = PLAN.replacen(
            "(§16.3; G-HEAD-7) |",
            "(§16.3; G-HEAD-7); 506 registered input missing or unreadable |",
            1,
        );
        let errs = check(FIXTURE, &plan, &[]).expect_err("proposed in plan");
        assert!(
            errs.iter()
                .any(|e| e.contains("is proposed but §8.4 lists it")),
            "{errs:?}"
        );
        // An active row the plan lacks; the 3xx row after the section does not count.
        let extra = format!(
            "{FIXTURE}\n[[refusal]]\ncode = \"FMINE-REF-507\"\nfamily = \"5xx\"\nkey = \"frozen\"\nexit_code = 6\n\
             status = \"active\"\nplan_text = \"frozen\"\nwhat = \"w\"\nfix = \"f\"\n"
        );
        let errs = check(&extra, PLAN, &[]).expect_err("active not in plan");
        assert!(
            errs.iter()
                .any(|e| e.contains("FMINE-REF-507") && e.contains("not in §8.4")),
            "{errs:?}"
        );
    }

    #[test]
    fn test_rule_b_exit_codes_follow_the_family_rule() {
        let bad = FIXTURE.replacen(
            "key = \"no_claim_in_domain\"\nexit_code = 3",
            "key = \"no_claim_in_domain\"\nexit_code = 4",
            1,
        );
        let errs = check(&bad, PLAN, &[]).expect_err("2xx -> 4");
        assert!(
            errs.iter()
                .any(|e| e.contains("FMINE-REF-201") && e.contains("breaks §30.1")),
            "{errs:?}"
        );
        // The listed exceptions pass (504 -> 7, 506 -> 5); moving 504 to 8 leaves its exception unused.
        let moved = FIXTURE.replacen("exit_code = 7", "exit_code = 8", 1);
        let errs = check(&moved, PLAN, &[]).expect_err("unused exception");
        assert!(
            errs.iter()
                .any(|e| e.contains("FMINE-REF-504 -> 7 matches no registry row")),
            "{errs:?}"
        );
    }

    #[test]
    fn test_rule_c_unregistered_tokens_fail_with_path_and_line() {
        let text = format!(
            "ok {}\nbad {}\nnot a token FMINE-REF-nnn",
            token(101),
            token(999)
        );
        let errs =
            check(FIXTURE, PLAN, &[("crates/x/CONTRACT.md", &text)]).expect_err("unregistered");
        assert_eq!(
            errs,
            vec![format!(
                "crates/x/CONTRACT.md:2: {} is not a registry code",
                token(999)
            )]
        );
        assert_eq!(
            code_tokens(&format!("{}0 and {}", token(101), token(102))),
            vec![(1, "1010".to_string()), (1, "102".to_string())]
        );
        // A family wildcard is not a token; a letter O among the digits leaves a short, failing token.
        assert_eq!(
            code_tokens(&format!("{}xx {}O2", token(3), token(5))),
            vec![(1, "5".to_string())]
        );
    }

    #[test]
    fn test_rule_d_no_fix_suggests_clamping() {
        let bad = FIXTURE.replacen(
            "fix = \"Put the file back.\"",
            "fix = \"Clamp or re-normalize.\"",
            1,
        );
        let errs = check(&bad, PLAN, &[]).expect_err("clamp");
        assert!(
            errs.iter()
                .any(|e| e.contains("FMINE-REF-506") && e.contains("\"clamp\"")),
            "{errs:?}"
        );
    }

    #[test]
    fn test_stale_or_hand_edited_catalogue_fails() {
        let sources: Vec<(String, String)> = Vec::new();
        // Mutation 1: one exit code changed in the registry; the committed catalogue is now stale.
        let edited = FIXTURE.replacen("exit_code = 5", "exit_code = 8", 1);
        let errs = check_texts(edited.as_bytes(), &catalog_for(FIXTURE), PLAN, &sources)
            .expect_err("stale");
        assert!(
            errs.iter().any(|e| e.contains("differs from what")),
            "{errs:?}"
        );
        // Mutation 2: one catalogue line edited by hand.
        let hand = catalog_for(FIXTURE).replacen("exit_code: 3,", "exit_code: 4,", 1);
        let errs = check_texts(FIXTURE.as_bytes(), &hand, PLAN, &sources).expect_err("hand edit");
        assert!(
            errs.iter().any(|e| e.contains("differs from what")),
            "{errs:?}"
        );
        // Even a comment-only change to the registry changes its digest and so the header.
        let comment = format!("# note\n{FIXTURE}");
        let errs = check_texts(comment.as_bytes(), &catalog_for(FIXTURE), PLAN, &sources)
            .expect_err("digest");
        assert!(
            errs.iter().any(|e| e.contains("differs from what")),
            "{errs:?}"
        );
    }

    #[test]
    fn test_committed_registry_catalogue_and_plan_agree() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = check_refusals(&root).expect("check-refusals passes on the workspace");
        assert_eq!(report.rows, 33);
        assert_eq!(
            (report.active, report.proposed, report.reserved),
            (25, 8, 0)
        );
        assert_eq!(report.plan_codes, 25);
        assert_eq!(report.warnings.len(), 8);
        assert!(report.tokens > 0 && report.files > 0);
    }
}
