//! `L-6` (plan PO-UNIT-2, BD-009, bd-u56b): no K or E public function takes a bare `f64` for a physical quantity.
//!
//! Deny by default. Every parameter typed exactly `f64` in a `pub fn` of a K or E crate's production code fails,
//! unless [`ALLOW_PATH`] holds a row naming its crate, file, function and parameter. A row's `kind` says whether the
//! value is `generic` (dimensionless, or a statistical function over values in the caller's own unit) or `debt` (a
//! physical quantity or money passed bare, still to be typed). A row that matches nothing fails, so the list cannot
//! outlive the code it excuses.
//!
//! The source is scanned as text, not parsed: line comments are skipped, and a signature may span lines. A
//! `pub fn` written inside a string literal would be read as code. `pub(crate)` and private functions, return types,
//! slices and struct fields are not checked.

use std::collections::BTreeSet;

use fmine_registry::toml_subset::{TomlRow, TomlValue, parse_subset};

/// The allow-list, relative to the workspace root.
pub const ALLOW_PATH: &str = "registries/l6_allow.toml";

/// The two kinds of allow-list row.
pub const KINDS: [&str; 2] = ["generic", "debt"];

const FIELDS: [&str; 6] = ["crate", "file", "fn", "params", "kind", "reason"];

/// One bare `f64` parameter of a `pub fn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareParam {
    /// 1-based line of the parameter.
    pub line: usize,
    /// The function's name.
    pub func: String,
    /// The parameter's name.
    pub param: String,
}

/// One `[[allow]]` row of [`ALLOW_PATH`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowRow {
    /// Line of the `[[allow]]` header.
    pub line: usize,
    /// Package name, e.g. `fmine-metrics`.
    pub krate: String,
    /// Path inside the crate, e.g. `src/lib.rs`.
    pub file: String,
    /// Function name.
    pub func: String,
    /// The excused parameters.
    pub params: Vec<String>,
    /// `generic` or `debt`.
    pub kind: String,
    /// Why the parameter may stay a bare `f64`.
    pub reason: String,
}

/// A scanned site: the crate, the file inside it, and the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// Package name.
    pub krate: String,
    /// Path inside the crate, with `/` separators.
    pub file: String,
    /// The parameter.
    pub bare: BareParam,
}

/// Removes `//` comments outside string literals, keeping every line so that offsets map to the same lines.
fn without_line_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.split('\n') {
        let bytes = line.as_bytes();
        let (mut in_str, mut escaped, mut cut) = (false, false, bytes.len());
        for (i, &b) in bytes.iter().enumerate() {
            if in_str {
                match b {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    b'"' => in_str = false,
                    _ => {}
                }
            } else if b == b'"' {
                in_str = true;
            } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
                cut = i;
                break;
            }
        }
        out.push_str(line.get(..cut).unwrap_or(line));
        out.push('\n');
    }
    out
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    i
}

/// The byte just past the bracket that closes the one at `open`, counting `()`, `[]`, `{}` and `<>`. A `>` after
/// `-` (an arrow) closes nothing.
fn matching_close(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b'>' if i > 0 && bytes.get(i - 1) == Some(&b'-') => {}
            b')' | b']' | b'}' | b'>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The start offsets of the depth-0 pieces between `start` and `end`, split at commas.
fn split_params(bytes: &[u8], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut pieces = Vec::new();
    let (mut depth, mut from) = (0_usize, start);
    for i in start..end {
        match bytes.get(i) {
            Some(b'(' | b'[' | b'{' | b'<') => depth += 1,
            Some(b'>') if i > 0 && bytes.get(i - 1) == Some(&b'-') => {}
            Some(b')' | b']' | b'}' | b'>') => depth = depth.saturating_sub(1),
            Some(b',') if depth == 0 => {
                pieces.push((from, i));
                from = i + 1;
            }
            _ => {}
        }
    }
    pieces.push((from, end));
    pieces
}

/// Every parameter typed exactly `f64` in a `pub fn` (also `pub const`, `async` or `unsafe fn`) of `prod_code`.
#[must_use]
pub fn bare_f64_params(prod_code: &str) -> Vec<BareParam> {
    let text = without_line_comments(prod_code);
    let bytes = text.as_bytes();
    let line_of = |at: usize| text.get(..at).map_or(1, |s| s.matches('\n').count() + 1);
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(off) = text.get(i..).and_then(|s| s.find("pub")) {
        let at = i + off;
        i = at + 3;
        if at > 0 && bytes.get(at - 1).copied().is_some_and(is_ident) {
            continue;
        }
        if !bytes.get(at + 3).is_some_and(u8::is_ascii_whitespace) {
            continue;
        }
        let mut j = skip_ws(bytes, at + 3);
        for qualifier in ["const", "async", "unsafe"] {
            if text.get(j..).is_some_and(|s| s.starts_with(qualifier))
                && bytes
                    .get(j + qualifier.len())
                    .is_some_and(u8::is_ascii_whitespace)
            {
                j = skip_ws(bytes, j + qualifier.len());
            }
        }
        if !(text.get(j..).is_some_and(|s| s.starts_with("fn"))
            && bytes.get(j + 2).is_some_and(u8::is_ascii_whitespace))
        {
            continue;
        }
        let name_start = skip_ws(bytes, j + 2);
        let mut name_end = name_start;
        while bytes.get(name_end).copied().is_some_and(is_ident) {
            name_end += 1;
        }
        let Some(func) = text.get(name_start..name_end).filter(|n| !n.is_empty()) else {
            continue;
        };
        let mut k = skip_ws(bytes, name_end);
        if bytes.get(k) == Some(&b'<') {
            let Some(after) = matching_close(bytes, k) else {
                continue;
            };
            k = skip_ws(bytes, after);
        }
        if bytes.get(k) != Some(&b'(') {
            continue;
        }
        let Some(close) = matching_close(bytes, k) else {
            continue;
        };
        for (from, to) in split_params(bytes, k + 1, close - 1) {
            let Some(piece) = text.get(from..to) else {
                continue;
            };
            let Some((name, ty)) = piece.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let name = name.strip_prefix("mut ").map_or(name, str::trim);
            if ty.trim() == "f64" && !name.is_empty() && name.bytes().all(is_ident) {
                let lead = piece.len() - piece.trim_start().len();
                found.push(BareParam {
                    line: line_of(from + lead),
                    func: func.to_string(),
                    param: name.to_string(),
                });
            }
        }
        i = close;
    }
    found
}

fn row_str<'a>(row: &'a TomlRow, key: &str) -> Result<&'a str, String> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) => Ok(s.as_str()),
        Some(TomlValue::Int(_)) => Err(format!(
            "{ALLOW_PATH}:{}: `{key}` must be a string",
            row.line
        )),
        None => Err(format!(
            "{ALLOW_PATH}:{}: [[allow]] lacks `{key}`",
            row.line
        )),
    }
}

/// Parses [`ALLOW_PATH`]. Every error is reported, each with its line.
pub fn parse_allow(text: &str) -> Result<Vec<AllowRow>, Vec<String>> {
    let doc = parse_subset(text).map_err(|e| vec![format!("{ALLOW_PATH}: {e}")])?;
    let mut errors = Vec::new();
    if let Some(f) = doc.top.first() {
        errors.push(format!(
            "{ALLOW_PATH}:{}: top-level key `{}`; only [[allow]] rows are read",
            f.line, f.key
        ));
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for row in &doc.rows {
        if row.table != "allow" {
            errors.push(format!(
                "{ALLOW_PATH}:{}: unknown table [[{}]]",
                row.line, row.table
            ));
            continue;
        }
        if let Some(extra) = row
            .fields
            .iter()
            .find(|f| !FIELDS.contains(&f.key.as_str()))
        {
            errors.push(format!(
                "{ALLOW_PATH}:{}: unknown field `{}`",
                extra.line, extra.key
            ));
            continue;
        }
        let fields: Result<Vec<&str>, String> = FIELDS.iter().map(|k| row_str(row, k)).collect();
        let [krate, file, func, params, kind, reason] = match fields.as_deref() {
            Ok(&[a, b, c, d, e, f]) => [a, b, c, d, e, f],
            Ok(_) => continue,
            Err(e) => {
                errors.push(e.clone());
                continue;
            }
        };
        if !KINDS.contains(&kind) {
            errors.push(format!(
                "{ALLOW_PATH}:{}: kind `{kind}` is not `generic` or `debt`",
                row.line
            ));
        }
        if reason.trim().is_empty() {
            errors.push(format!("{ALLOW_PATH}:{}: empty reason", row.line));
        }
        let params: Vec<String> = params
            .split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if params.is_empty() {
            errors.push(format!("{ALLOW_PATH}:{}: no params", row.line));
        }
        for p in &params {
            if !seen.insert((
                krate.to_string(),
                file.to_string(),
                func.to_string(),
                p.clone(),
            )) {
                errors.push(format!(
                    "{ALLOW_PATH}:{}: `{krate}` {file} `{func}({p})` is listed twice",
                    row.line
                ));
            }
        }
        rows.push(AllowRow {
            line: row.line,
            krate: krate.to_string(),
            file: file.to_string(),
            func: func.to_string(),
            params,
            kind: kind.to_string(),
            reason: reason.to_string(),
        });
    }
    if errors.is_empty() {
        Ok(rows)
    } else {
        Err(errors)
    }
}

/// Every site no row excuses, and every row parameter that matches no site.
#[must_use]
pub fn evaluate(sites: &[Site], rows: &[AllowRow]) -> Vec<String> {
    let allowed = |s: &Site| {
        rows.iter().any(|r| {
            r.krate == s.krate
                && r.file == s.file
                && r.func == s.bare.func
                && r.params.contains(&s.bare.param)
        })
    };
    let mut errors: Vec<String> = sites
        .iter()
        .filter(|s| !allowed(s))
        .map(|s| {
            format!(
                "crates/{}/{}:{}: `{}: f64` in K/E public fn `{}` (L-6): take a typed quantity (fmine-units) or a \
                 Claim; only a dimensionless or unit-generic value may get a `generic` row in {ALLOW_PATH}",
                s.krate, s.file, s.bare.line, s.bare.param, s.bare.func
            )
        })
        .collect();
    for r in rows {
        for p in &r.params {
            let hit = sites.iter().any(|s| {
                s.krate == r.krate
                    && s.file == r.file
                    && s.bare.func == r.func
                    && &s.bare.param == p
            });
            if !hit {
                errors.push(format!(
                    "{ALLOW_PATH}:{}: crates/{}/{} has no `pub fn {}` with a bare `{p}: f64`; remove it from the row",
                    r.line, r.krate, r.file, r.func
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(code: &str) -> Vec<(usize, String, String)> {
        bare_f64_params(code)
            .into_iter()
            .map(|b| (b.line, b.func, b.param))
            .collect()
    }

    fn site(krate: &str, file: &str, func: &str, param: &str) -> Site {
        Site {
            krate: krate.to_string(),
            file: file.to_string(),
            bare: BareParam {
                line: 1,
                func: func.to_string(),
                param: param.to_string(),
            },
        }
    }

    #[test]
    fn test_l6_any_bare_f64_parameter_fails_on_one_line_or_several() {
        assert_eq!(
            names("pub fn f(tonnes: f64) -> f64 { tonnes }"),
            [(1, "f".to_string(), "tonnes".to_string())]
        );
        assert_eq!(
            names(
                "impl A {\n    pub fn g(\n        &self,\n        grade: f64,\n        n: usize,\n    ) {}\n}"
            ),
            [(4, "g".to_string(), "grade".to_string())]
        );
        assert_eq!(
            names("pub const fn h(mut x: f64, y: f64) {}\npub async fn k(z: f64) {}"),
            [
                (1, "h".to_string(), "x".to_string()),
                (1, "h".to_string(), "y".to_string()),
                (2, "k".to_string(), "z".to_string()),
            ]
        );
        assert_eq!(
            names(
                "pub fn solve<F: FnMut(f64) -> f64>(cost: f64, f: F) -> f64 where F: Fn(f64) -> f64 { 0.0 }"
            ),
            [(1, "solve".to_string(), "cost".to_string())]
        );
    }

    #[test]
    fn test_l6_typed_private_and_commented_parameters_pass() {
        let code = "\
/// pub fn doc(x: f64) {}
// pub fn commented(x: f64) {}
pub fn typed(t: Qty<Mass, Dry>, n: usize, v: &[f64], c: Claim<f64>) -> f64 { 0.0 }
pub(crate) fn inner(x: f64) {}
fn private(x: f64) {}
pub fn closure(f: impl Fn(f64) -> f64, pair: (f64, f64)) {}
pub struct S { pub mass: f64 }
let s = \"https://example.org\"; pub fn after(y: f64) {}
";
        assert_eq!(names(code), [(8, "after".to_string(), "y".to_string())]);
    }

    #[test]
    fn test_l6_allow_rows_excuse_their_sites_and_refuse_stale_or_empty_ones() {
        let text = "\
[[allow]]
crate = \"fmine-metrics\"
file = \"src/lib.rs\"
fn = \"compute_geo\"
params = \"gold_koz, copper_mlb\"
kind = \"debt\"
reason = \"gold in koz and copper in Mlb, passed bare (bd-u56b)\"
";
        let rows = parse_allow(text).expect("rows");
        let geo = |p: &str| site("fmine-metrics", "src/lib.rs", "compute_geo", p);
        assert!(evaluate(&[geo("gold_koz"), geo("copper_mlb")], &rows).is_empty());

        // A site no row names fails, naming the file, line and parameter.
        let other = site("fmine-metrics", "src/lib.rs", "compute", "depletion_moz");
        let errs = evaluate(&[geo("gold_koz"), geo("copper_mlb"), other], &rows);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(
            errs[0].starts_with("crates/fmine-metrics/src/lib.rs:1: `depletion_moz: f64`"),
            "{errs:?}"
        );

        // A row parameter that matches no site, or a site in another file, is stale.
        let moved = site("fmine-metrics", "src/geo.rs", "compute_geo", "gold_koz");
        let errs = evaluate(&[moved, geo("copper_mlb")], &rows);
        assert_eq!(errs.len(), 2, "{errs:?}");
        assert!(
            errs.iter()
                .any(|e| e.contains("has no `pub fn compute_geo` with a bare `gold_koz: f64`"))
        );

        // Empty reason, unknown kind, missing field, unknown field and a repeated parameter are refused.
        let bad = text.replace("kind = \"debt\"", "kind = \"later\"").replace(
            "reason = \"gold in koz and copper in Mlb, passed bare (bd-u56b)\"",
            "reason = \" \"",
        );
        let errs = parse_allow(&bad).unwrap_err();
        assert!(errs.iter().any(|e| e.ends_with("empty reason")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("kind `later`")), "{errs:?}");
        let errs = parse_allow(&text.replace("file = \"src/lib.rs\"\n", "")).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("lacks `file`")), "{errs:?}");
        let errs = parse_allow(&text.replace("kind = ", "why = ")).unwrap_err();
        assert!(
            errs.iter().any(|e| e.contains("unknown field `why`")),
            "{errs:?}"
        );
        let errs = parse_allow(&format!("{text}{text}")).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("listed twice")), "{errs:?}");
    }
}
