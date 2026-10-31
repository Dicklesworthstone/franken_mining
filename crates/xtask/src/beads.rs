//! `check-beads`: each close reason cites its own evidence, and no bead closes over an open blocker (BD-324, §39,
//! RISK-33).
//!
//! The check reads the committed `.beads/issues.jsonl` (written by `br sync --flush-only`). Each closed bead must
//! pass these rules:
//! - R-SHARED: no other closed bead holds the same reason once both are lowercased, whitespace is collapsed and
//!   bead ids read `bd-#`;
//! - R-TYPED: the reason holds a `test:`, `record:`, `ci:`, `decision:` or `commit:` reference;
//! - R-RESOLVE: each reference resolves:
//!   - a `test:` reference's last `::` segment is a `fn` under `crates/`. Trailing punctuation, an `@<sha>`
//!     suffix and the `{` opening an `{a, b}` group are not part of the name;
//!   - a `record:` path is a file;
//!   - `ci:<run>@<sha>` and `commit:<sha>` are well formed. They are not looked up, because CI checks out at
//!     depth 1;
//! - R-GATE: each `GATE-…` token has a `docs/delivery/<token>*.md` record with a `Verdict: PASS` line;
//! - R-BLOCKED: every bead it depends on through a `blocks` edge is closed.
//!
//! Ten closes within 60 s is a warning (R-WINDOW), because re-closing with distinct evidence is legitimate.
//! Whether the evidence proves the done-when is a question for review (BD-357), not for this check.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The tracker export this check reads.
pub const ISSUES_PATH: &str = ".beads/issues.jsonl";
/// The typed reference kinds of a close reason (BD-323's format).
pub const KINDS: [&str; 5] = ["test", "record", "ci", "decision", "commit"];
const WINDOW_SECS: i64 = 60;
const WINDOW_MIN: usize = 10;

/// What a clean tracker reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeadsReport {
    pub rows: usize,
    pub closed: usize,
    /// Distinct normalised close reasons: equal to `closed` whenever R-SHARED holds.
    pub distinct: usize,
    pub warnings: Vec<String>,
}

impl BeadsReport {
    /// `{"check":"check-beads","rows","closed","distinct"}`.
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"check-beads\",\"rows\":{},\"closed\":{},\"distinct\":{}}}",
            self.rows, self.closed, self.distinct
        )
    }
}

/// [`evaluate`] over the workspace: `fn` names from every `.rs` file under `crates/`, and the
/// `docs/delivery/*.md` records.
pub fn check_beads(root: &Path) -> Result<BeadsReport, Vec<String>> {
    let jsonl = fs::read_to_string(root.join(ISSUES_PATH))
        .map_err(|e| vec![format!("cannot read {ISSUES_PATH}: {e}")])?;
    let mut fns = BTreeSet::new();
    for path in crate::collect_rs_files(&root.join("crates")) {
        let text = fs::read_to_string(&path)
            .map_err(|e| vec![format!("cannot read {}: {e}", path.display())])?;
        fns.extend(fn_names(&text));
    }
    let mut records = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("docs/delivery")) {
        for entry in entries.flatten() {
            if let Ok(text) = fs::read_to_string(entry.path()) {
                records.push((entry.file_name().to_string_lossy().into_owned(), text));
            }
        }
    }
    evaluate(&jsonl, &fns, &|p| root.join(p).is_file(), &records)
}

/// The rules over an `issues.jsonl` text. `fns` are the `fn` names under `crates/`, `is_file` answers for a
/// repo-relative path, and `records` are the delivery records as (file name, text).
pub fn evaluate(
    jsonl: &str,
    fns: &BTreeSet<String>,
    is_file: &dyn Fn(&str) -> bool,
    records: &[(String, String)],
) -> Result<BeadsReport, Vec<String>> {
    let rows = parse_rows(jsonl)?;
    let status: BTreeMap<&str, &str> = rows
        .iter()
        .map(|r| (r.id.as_str(), r.status.as_str()))
        .collect();
    let closed: Vec<&Row> = rows.iter().filter(|r| r.status == "closed").collect();
    let mut errors = Vec::new();
    let mut by_reason: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for r in &closed {
        by_reason
            .entry(normalise(&r.close_reason))
            .or_default()
            .push(&r.id);
        let refs = typed_refs(&r.close_reason);
        if refs.is_empty() {
            errors.push(format!(
                "R-TYPED: {} has no typed reference ({}:)",
                r.id,
                KINDS.join(":, ")
            ));
        }
        for (kind, value) in refs {
            if let Some(why) = unresolved(kind, value, fns, is_file) {
                errors.push(format!("R-RESOLVE: {} cites {kind}:{value}: {why}", r.id));
            }
        }
        for gate in gate_tokens(&r.close_reason) {
            let passed = |(name, text): &(String, String)| {
                name.starts_with(gate) && name.ends_with(".md") && has_pass_verdict(text)
            };
            if !records.iter().any(passed) {
                errors.push(format!(
                    "R-GATE: {} cites {gate}, and no docs/delivery/{gate}*.md has a `Verdict: PASS` line",
                    r.id
                ));
            }
        }
        for b in &r.blocks {
            match status.get(b.as_str()) {
                Some(&"closed") => {}
                s => errors.push(format!(
                    "R-BLOCKED: {} is closed, and its blocker {b} is {}",
                    r.id,
                    s.unwrap_or(&"not in the tracker")
                )),
            }
        }
    }
    for ids in by_reason.values().filter(|ids| ids.len() > 1) {
        errors.push(format!(
            "R-SHARED: {} closed beads share one reason: {}",
            ids.len(),
            ids.join(", ")
        ));
    }
    let warnings = window_warnings(&closed);
    if errors.is_empty() {
        return Ok(BeadsReport {
            rows: rows.len(),
            closed: closed.len(),
            distinct: by_reason.len(),
            warnings,
        });
    }
    let count = |rule: &str| errors.iter().filter(|e| e.starts_with(rule)).count();
    let counts = ["R-SHARED", "R-TYPED", "R-RESOLVE", "R-GATE", "R-BLOCKED"]
        .map(|r| format!("{r} {}", count(r)));
    errors.push(format!(
        "counts: {}; closed {}",
        counts.join(", "),
        closed.len()
    ));
    errors.extend(warnings.into_iter().map(|w| format!("(warning) {w}")));
    Err(errors)
}

struct Row {
    id: String,
    status: String,
    close_reason: String,
    closed_at: Option<String>,
    /// `depends_on_id` of each `blocks` dependency.
    blocks: Vec<String>,
}

fn parse_rows(jsonl: &str) -> Result<Vec<Row>, Vec<String>> {
    let (mut rows, mut errors) = (Vec::new(), Vec::new());
    for (n, line) in jsonl
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        match parse_json(line).and_then(|v| row(&v)) {
            Ok(r) => rows.push(r),
            Err(e) => errors.push(format!("{ISSUES_PATH} line {}: {e}", n + 1)),
        }
    }
    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

fn row(v: &Json) -> Result<Row, String> {
    let text = |j: Option<&Json>| match j {
        Some(Json::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let mut blocks = Vec::new();
    if let Some(Json::Arr(deps)) = v.get("dependencies") {
        for d in deps
            .iter()
            .filter(|d| text(d.get("type")).as_deref() == Some("blocks"))
        {
            blocks.push(
                text(d.get("depends_on_id"))
                    .ok_or("a `blocks` dependency has no `depends_on_id`")?,
            );
        }
    }
    Ok(Row {
        id: text(v.get("id")).ok_or("no string `id`")?,
        status: text(v.get("status")).ok_or("no string `status`")?,
        close_reason: text(v.get("close_reason")).unwrap_or_default(),
        closed_at: text(v.get("closed_at")),
        blocks,
    })
}

/// A JSON value; numbers are checked for shape only, since no rule reads one.
#[derive(Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num,
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(kv) => kv.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

fn parse_json(line: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: line.as_bytes(),
        i: 0,
    };
    let v = p.value(0)?;
    p.ws();
    if p.i == p.b.len() {
        Ok(v)
    } else {
        Err(format!("trailing bytes at byte {}", p.i))
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.b.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) != Some(&c) {
            return Err(format!("expected `{}` at byte {}", c as char, self.i));
        }
        self.i += 1;
        Ok(())
    }

    /// The items of an array or object up to `close`, each read by `item`.
    fn items(
        &mut self,
        close: u8,
        mut item: impl FnMut(&mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        self.i += 1;
        self.ws();
        if self.b.get(self.i) == Some(&close) {
            self.i += 1;
            return Ok(());
        }
        loop {
            item(self)?;
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(&c) if c == close => break,
                _ => {
                    return Err(format!(
                        "expected `,` or `{}` at byte {}",
                        close as char, self.i
                    ));
                }
            }
        }
        self.i += 1;
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > 64 {
            return Err("nesting deeper than 64".into());
        }
        self.ws();
        let lit = |p: &mut Self, word: &str, v: Json| {
            if p.b[p.i..].starts_with(word.as_bytes()) {
                p.i += word.len();
                Ok(v)
            } else {
                Err(format!("bad literal at byte {}", p.i))
            }
        };
        match self.b.get(self.i) {
            Some(b'{') => {
                let mut kv = Vec::new();
                self.items(b'}', |p| {
                    p.ws();
                    let k = p.string()?;
                    p.eat(b':')?;
                    kv.push((k, p.value(depth + 1)?));
                    Ok(())
                })?;
                Ok(Json::Obj(kv))
            }
            Some(b'[') => {
                let mut items = Vec::new();
                self.items(b']', |p| {
                    items.push(p.value(depth + 1)?);
                    Ok(())
                })?;
                Ok(Json::Arr(items))
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => lit(self, "true", Json::Bool(true)),
            Some(b'f') => lit(self, "false", Json::Bool(false)),
            Some(b'n') => lit(self, "null", Json::Null),
            Some(c) if *c == b'-' || c.is_ascii_digit() => {
                while self
                    .b
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_digit() || b"+-.eE".contains(c))
                {
                    self.i += 1;
                }
                Ok(Json::Num)
            }
            _ => Err(format!("unexpected byte at {}", self.i)),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.b.get(self.i) != Some(&b'"') {
            return Err(format!("expected a string at byte {}", self.i));
        }
        self.i += 1;
        let mut buf = Vec::new();
        loop {
            let at = self.i;
            self.i += 1;
            match self.b.get(at) {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    return String::from_utf8(buf).map_err(|e| format!("invalid UTF-8: {e}"));
                }
                Some(&c) if c < 0x20 => {
                    return Err(format!("raw control byte in a string at byte {at}"));
                }
                Some(b'\\') => {
                    let c = match self.b.get(self.i) {
                        Some(b'u') => self.unicode()?,
                        Some(&e @ (b'"' | b'\\' | b'/')) => char::from(e),
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        Some(b'r') => '\r',
                        Some(b'b') => '\u{8}',
                        Some(b'f') => '\u{c}',
                        _ => return Err(format!("bad escape at byte {at}")),
                    };
                    self.i += 1;
                    buf.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                }
                Some(&c) => buf.push(c),
            }
        }
    }

    /// `\uXXXX` with `self.i` on the `u`, a surrogate pair included; leaves `self.i` on the last hex digit.
    fn unicode(&mut self) -> Result<char, String> {
        let at = self.i;
        let hex4 = |p: &Self, from: usize| {
            let h =
                p.b.get(from..from + 4)
                    .filter(|h| h.iter().all(u8::is_ascii_hexdigit))?;
            u32::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
        };
        let bad = || format!("bad \\u escape at byte {at}");
        let hi = hex4(self, self.i + 1).ok_or_else(bad)?;
        self.i += 4;
        let code = if (0xD800..0xDC00).contains(&hi) {
            let lo = (self.b.get(self.i + 1..self.i + 3) == Some(b"\\u".as_slice()))
                .then(|| hex4(self, self.i + 3))
                .flatten()
                .filter(|lo| (0xDC00..0xE000).contains(lo))
                .ok_or_else(bad)?;
            self.i += 6;
            0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
        } else {
            hi
        };
        char::from_u32(code).ok_or_else(bad)
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Lowercase, whitespace collapsed, and each `bd-…` id read as `bd-#`.
fn normalise(reason: &str) -> String {
    let lower = reason.to_lowercase();
    let mut words = Vec::new();
    for word in lower.split_whitespace() {
        let (mut out, mut rest) = (String::new(), word);
        while let Some(i) = rest.find("bd-") {
            let boundary = rest[..i]
                .chars()
                .last()
                .or_else(|| out.chars().last())
                .is_none_or(|c| !is_word(c));
            out.push_str(&rest[..i + 3]);
            rest = &rest[i + 3..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
                .unwrap_or(rest.len());
            if boundary && end > 0 {
                out.push('#');
                rest = &rest[end..];
            }
        }
        out.push_str(rest);
        words.push(out);
    }
    words.join(" ")
}

/// `\b(test|record|ci|decision|commit):[^\s;,]+`, left to right and not overlapping, as a regex finds them: the
/// `record::tests` inside `test:fmine-calib::record::tests::f` is part of that test reference.
fn typed_refs(reason: &str) -> Vec<(&'static str, &str)> {
    let mut refs = Vec::new();
    for kind in KINDS {
        let tag = format!("{kind}:");
        for (i, _) in reason.match_indices(&tag) {
            if reason[..i].chars().last().is_some_and(is_word) {
                continue;
            }
            let rest = &reason[i + tag.len()..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == ';' || c == ',')
                .unwrap_or(rest.len());
            if end > 0 {
                refs.push((i, kind, &rest[..end]));
            }
        }
    }
    refs.sort_by_key(|r| r.0);
    let mut end = 0;
    let mut out = Vec::new();
    for (i, kind, value) in refs {
        if i >= end {
            end = i + kind.len() + 1 + value.len();
            out.push((kind, value));
        }
    }
    out
}

/// Why a reference does not resolve, when it does not.
fn unresolved(
    kind: &str,
    value: &str,
    fns: &BTreeSet<String>,
    is_file: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let v = value.trim_end_matches(['.', ':', ')', '`', '\'', '"']);
    let sha = |s: &str| (7..=40).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit());
    match kind {
        "test" => {
            // `p{a,b}` names `pa` and `pb`; only `pa` reaches here, since the reference ends at the `,`.
            let path = v.split('@').next().unwrap_or(v);
            let name = path.rsplit("::").next().unwrap_or(path).replace('{', "");
            (!fns.contains(&name)).then(|| format!("no `fn {name}(` under crates/"))
        }
        "record" => {
            let path = v.split('#').next().unwrap_or(v);
            let escapes = path.starts_with('/') || path.split('/').any(|s| s == "..");
            (escapes || !is_file(path)).then(|| format!("no file {path}"))
        }
        "ci" => {
            let ok = v.split_once('@').is_some_and(|(run, s)| {
                !run.is_empty() && run.bytes().all(|b| b.is_ascii_digit()) && sha(s)
            });
            (!ok).then(|| "not <run id>@<sha>".to_string())
        }
        "commit" => (!sha(v)).then(|| "not a 7 to 40 digit hex sha".to_string()),
        _ => None,
    }
}

fn gate_tokens(reason: &str) -> BTreeSet<&str> {
    let gate = |(i, _): (usize, &str)| {
        let rest = &reason[i..];
        let end = rest
            .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'))
            .unwrap_or(rest.len());
        let token = rest[..end].trim_end_matches('-');
        (token.len() > "GATE-".len()).then_some(token)
    };
    reason.match_indices("GATE-").filter_map(gate).collect()
}

/// A line opening `Verdict:` whose verdict is the whole token `PASS`: `Verdict: NOT PASS` and `Verdict: PASSED`
/// are not passes.
fn has_pass_verdict(text: &str) -> bool {
    text.lines().any(|l| {
        let verdict = l.strip_prefix("Verdict:").map(str::trim_start);
        verdict
            .and_then(|v| v.strip_prefix("PASS"))
            .is_some_and(|r| !r.starts_with(is_word))
    })
}

/// The name after each `fn ` that opens a word.
fn fn_names(text: &str) -> Vec<String> {
    let starts = text
        .match_indices("fn ")
        .filter(|(i, _)| !text[..*i].chars().last().is_some_and(is_word));
    let name = |(i, _): (usize, &str)| -> String {
        text[i + 3..]
            .trim_start()
            .chars()
            .take_while(|c| is_word(*c))
            .collect()
    };
    starts
        .map(name)
        .filter(|n: &String| !n.is_empty())
        .collect()
}

/// Clusters of at least [`WINDOW_MIN`] closes within [`WINDOW_SECS`].
fn window_warnings(closed: &[&Row]) -> Vec<String> {
    let mut t: Vec<(i64, &str)> = closed
        .iter()
        .filter_map(|r| Some((epoch_secs(r.closed_at.as_deref()?)?, r.id.as_str())))
        .collect();
    t.sort_unstable();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < t.len() {
        let n = t[i..]
            .iter()
            .take_while(|(s, _)| s - t[i].0 <= WINDOW_SECS)
            .count();
        if n < WINDOW_MIN {
            i += 1;
            continue;
        }
        let (first, last) = (t[i], t[i + n - 1]);
        out.push(format!(
            "R-WINDOW: {n} closes within {} s, {} to {}",
            last.0 - first.0,
            first.1,
            last.1
        ));
        i += n;
    }
    out
}

/// Seconds since 1970 of an RFC 3339 UTC time such as `2026-10-30T04:18:59.167136Z`; the fraction is dropped.
fn epoch_secs(ts: &str) -> Option<i64> {
    let n = |from: usize, to: usize| ts.get(from..to)?.parse::<i64>().ok();
    let (y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let y = if m <= 2 { y - 1 } else { y };
    let (era, yoe) = (y.div_euclid(400), y.rem_euclid(400));
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    Some(
        (era * 146_097 + doe - 719_468) * 86_400 + n(11, 13)? * 3600 + n(14, 16)? * 60 + n(17, 19)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: &str, status: &str, reason: &str, blocks: &[&str]) -> String {
        let deps: Vec<String> = blocks
            .iter()
            .map(|b| {
                format!("{{\"issue_id\":\"{id}\",\"depends_on_id\":\"{b}\",\"type\":\"blocks\"}}")
            })
            .collect();
        format!(
            "{{\"id\":\"{id}\",\"status\":\"{status}\",\"close_reason\":\"{reason}\",\"closed_at\":\"2026-10-30T00:00:00Z\",\"dependencies\":[{}]}}",
            deps.join(",")
        )
    }

    fn run(lines: &[String], records: &[(&str, &str)]) -> Result<BeadsReport, Vec<String>> {
        let fns: BTreeSet<String> = ["test_golden_hash_10k_inputs".to_string()].into();
        let records: Vec<(String, String)> = records
            .iter()
            .map(|(n, t)| (n.to_string(), t.to_string()))
            .collect();
        // Any path ending `docs/real.md` "exists", so only the escape guard refuses `../docs/real.md`.
        evaluate(
            &lines.join("\n"),
            &fns,
            &|p| p.ends_with("docs/real.md"),
            &records,
        )
    }

    const GOOD: &str = "test:fmine-core::det::tests::test_golden_hash_10k_inputs";

    #[test]
    fn test_shared_reason_fails_and_names_every_holder() {
        let (a, b) = (
            format!("Done for bd-1: {GOOD}"),
            format!("done  for BD-2.3:  {GOOD}"),
        );
        let errs = run(
            &[
                line("bd-1", "closed", &a, &[]),
                line("bd-2.3", "closed", &b, &[]),
            ],
            &[],
        )
        .unwrap_err();
        assert_eq!(
            errs[0],
            "R-SHARED: 2 closed beads share one reason: bd-1, bd-2.3"
        );
        let c = format!("Other evidence: {GOOD}");
        let ok = run(
            &[
                line("bd-1", "closed", &a, &[]),
                line("bd-2", "closed", &c, &[]),
            ],
            &[],
        )
        .unwrap();
        assert_eq!((ok.closed, ok.distinct), (2, 2));
    }

    #[test]
    fn test_reason_resolution_by_kind() {
        let reason = |r: &str| run(&[line("bd-1", "closed", r, &[])], &[]);
        assert!(
            reason("ran cargo test --workspace; all green").unwrap_err()[0]
                .starts_with("R-TYPED: bd-1")
        );
        for ok in [
            GOOD,
            "test:xtask::tests::{test_golden_hash_10k_inputs, other}",
            "test:xtask::tests::test_golden_{hash_10k_inputs,other}",
            "test:fmine-calib::record::tests::test_golden_hash_10k_inputs",
            "test:a::test_golden_hash_10k_inputs@cd423b9.",
            "record:docs/real.md#E1; ci:41237995169@84f32ac, commit:84f32ac. decision:D-1",
        ] {
            assert!(reason(ok).is_ok(), "{ok}");
        }
        for (bad, why) in [
            (
                "test:fmine-core::det::tests::no_such_fn",
                "no `fn no_such_fn(` under crates/",
            ),
            ("test:fmine-public", "no `fn fmine-public(` under crates/"),
            ("record:docs/missing.md", "no file docs/missing.md"),
            ("record:../docs/real.md", "no file ../docs/real.md"),
            ("record:/docs/real.md", "no file /docs/real.md"),
            ("ci:abc@84f32ac", "not <run id>@<sha>"),
            ("commit:84f32", "not a 7 to 40 digit hex sha"),
        ] {
            let errs = reason(&format!("{GOOD} and {bad}")).unwrap_err();
            assert_eq!(errs[0], format!("R-RESOLVE: bd-1 cites {bad}: {why}"));
        }
    }

    #[test]
    fn test_gate_citation_needs_a_pass_record() {
        let rows = [line(
            "bd-1",
            "closed",
            &format!("GATE-BULL-01 met: {GOOD}"),
            &[],
        )];
        let missing = "R-GATE: bd-1 cites GATE-BULL-01, and no docs/delivery/GATE-BULL-01*.md has a `Verdict: PASS` line";
        assert_eq!(run(&rows, &[]).unwrap_err()[0], missing);
        assert!(run(&rows, &[("GATE-BULL-01.md", "# G\nVerdict: PASS\n")]).is_ok());
        for text in [
            "Verdict: NOT PASS",
            "Verdict: PASSED",
            "Verdict: FAIL (was PASS)",
            " Verdict: PASS",
        ] {
            assert_eq!(
                run(&rows, &[("GATE-BULL-01-record.md", text)]).unwrap_err()[0],
                missing,
                "{text}"
            );
        }
    }

    #[test]
    fn test_close_over_open_blocker_fails() {
        let rows = [
            line("bd-053", "closed", GOOD, &["bd-080"]),
            line("bd-080", "open", "", &[]),
        ];
        assert_eq!(
            run(&rows, &[]).unwrap_err()[0],
            "R-BLOCKED: bd-053 is closed, and its blocker bd-080 is open"
        );
        let rows = [
            line("bd-053", "closed", GOOD, &["bd-080"]),
            line("bd-080", "closed", "test:a::b", &[]),
        ];
        assert!(run(&rows, &[]).unwrap_err()[0].starts_with("R-RESOLVE: bd-080"));
    }

    #[test]
    fn test_escapes_parse_and_malformed_lines_are_numbered() {
        // The `\u` escapes are built at run time so that the source holds none for a tool to decode.
        let u = |hex: &str| format!("{}u{hex}", '\\');
        let text = format!(
            r#"{{"close_reason":"a \"q\" \\ {} {}{} \/","n":[1,-2.5e3,true,null]}}"#,
            u("00e9"),
            u("d83d"),
            u("de00")
        );
        assert!(text.contains(&u("d83d")) && text.is_ascii(), "{text}");
        let want = format!(
            "a \"q\" \\ {} {} /",
            char::from(0xe9_u8),
            char::from_u32(0x1f600).unwrap_or('?')
        );
        assert_eq!(
            parse_json(&text).unwrap().get("close_reason"),
            Some(&Json::Str(want))
        );
        let text = [
            line("bd-1", "closed", GOOD, &[]),
            "{\"id\":\"bd-2\",\"status\":\"clo".into(),
        ];
        assert_eq!(
            run(&text, &[]).unwrap_err(),
            [format!("{ISSUES_PATH} line 2: unterminated string")]
        );
        for bad in [
            r#"{"id":"a\ud800"}"#,
            r#"{"id":"a\q"}"#,
            r#"{"id":"a"} x"#,
            r#"{"id":"a",}"#,
        ] {
            assert!(parse_json(bad).is_err(), "{bad}");
        }
    }

    /// Today's defect pattern in miniature: four beads sharing one untyped reason that cites two gates, one of them
    /// closed over an open blocker.
    #[test]
    fn test_golden_negative_control() {
        let reason = "Closed for bd-X: GATE-LAYERS-00 and GATE-BULL-01 pass";
        let blocks = |id: &str| {
            if id == "bd-053" {
                vec!["bd-080"]
            } else {
                Vec::new()
            }
        };
        let mut rows: Vec<String> = ["bd-053", "bd-056", "bd-117", "bd-275"]
            .iter()
            .map(|id| line(id, "closed", &reason.replace("bd-X", id), &blocks(id)))
            .collect();
        rows.push(line("bd-080", "open", "", &[]));
        let mut expected = Vec::new();
        for id in ["bd-053", "bd-056", "bd-117", "bd-275"] {
            expected.push(format!(
                "R-TYPED: {id} has no typed reference (test:, record:, ci:, decision:, commit:)"
            ));
            for g in ["GATE-BULL-01", "GATE-LAYERS-00"] {
                expected.push(format!("R-GATE: {id} cites {g}, and no docs/delivery/{g}*.md has a `Verdict: PASS` line"));
            }
            if id == "bd-053" {
                expected.push("R-BLOCKED: bd-053 is closed, and its blocker bd-080 is open".into());
            }
        }
        expected.push(
            "R-SHARED: 4 closed beads share one reason: bd-053, bd-056, bd-117, bd-275".into(),
        );
        expected.push(
            "counts: R-SHARED 1, R-TYPED 4, R-RESOLVE 0, R-GATE 8, R-BLOCKED 1; closed 4".into(),
        );
        assert_eq!(run(&rows, &[]).unwrap_err(), expected);
    }

    #[test]
    fn test_ten_closes_in_a_minute_warn() {
        let at = |id: usize, s: usize| {
            line(&format!("bd-{id}"), "closed", &format!("{GOOD} {id}"), &[])
                .replace("00:00:00Z", &format!("23:59:{s:02}Z"))
        };
        let rows: Vec<String> = (0..10).map(|k| at(k, k * 6)).collect();
        let report = run(&rows, &[]).unwrap();
        assert_eq!(
            report.warnings,
            ["R-WINDOW: 10 closes within 54 s, bd-0 to bd-9"]
        );
        assert!(run(&rows[..9], &[]).unwrap().warnings.is_empty());
        assert_eq!(
            epoch_secs("2026-03-01T00:00:00Z")
                .zip(epoch_secs("2026-02-28T23:59:59Z"))
                .map(|(a, b)| a - b),
            Some(1)
        );
        assert_eq!(epoch_secs("1970-01-01T00:00:00.5Z"), Some(0));
    }
}
