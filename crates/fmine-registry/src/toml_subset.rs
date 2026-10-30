//! A strict subset of TOML for registries that are data (`NEW-vintage.S1`, bd-s9j3.1).
//!
//! Accepted, line by line:
//! - blank lines and `#` comments, also after a value;
//! - `[[name]]`, starting a new row of the array of tables `name`;
//! - `key = "basic string"` with the escapes `\"` and `\\` only;
//! - `key = 123_456`: a decimal integer, `_` only between digits, no leading zero, optional `-`.
//!
//! Everything else is refused with its 1-based line number: plain `[table]` headers, dotted or quoted keys,
//! inline tables, arrays, literal and multi-line strings, other escapes, floats, booleans, dates and duplicate
//! keys within one row. Refusing rather than guessing keeps a registry edit from meaning something other than
//! what it says.

use std::fmt;

/// A value the subset admits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TomlValue {
    Str(String),
    Int(i64),
}

/// One `key = value` with the line it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlField {
    pub key: String,
    pub value: TomlValue,
    pub line: usize,
}

/// One `[[name]]` row and its fields, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlRow {
    pub table: String,
    pub line: usize,
    pub fields: Vec<TomlField>,
}

impl TomlRow {
    /// The field named `key`, if present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&TomlField> {
        self.fields.iter().find(|f| f.key == key)
    }
}

/// A parsed document: the keys before the first `[[name]]`, then every row in file order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TomlDoc {
    pub top: Vec<TomlField>,
    pub rows: Vec<TomlRow>,
}

impl TomlDoc {
    /// The rows of table `name`, in file order.
    pub fn rows_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a TomlRow> + 'a {
        self.rows.iter().filter(move |r| r.table == name)
    }

    /// The top-level field named `key`, if present.
    #[must_use]
    pub fn top_get(&self, key: &str) -> Option<&TomlField> {
        self.top.iter().find(|f| f.key == key)
    }
}

/// Why a line is outside the subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsetError {
    /// 1-based line number.
    pub line: usize,
    pub reason: String,
}

impl fmt::Display for SubsetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for SubsetError {}

fn is_bare_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Parses `text`, refusing the first line outside the subset.
pub fn parse_subset(text: &str) -> Result<TomlDoc, SubsetError> {
    let mut doc = TomlDoc::default();
    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let err = |reason: String| SubsetError { line, reason };
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if let Some(rest) = s.strip_prefix("[[") {
            let Some((name, tail)) = rest.split_once("]]") else {
                return Err(err("unterminated `[[` header".to_string()));
            };
            let name = name.trim();
            if !is_bare_key(name) {
                return Err(err(format!(
                    "table name {name:?} is not a bare key (dotted or quoted names are outside the subset)"
                )));
            }
            only_comment(tail).map_err(err)?;
            doc.rows.push(TomlRow {
                table: name.to_string(),
                line,
                fields: Vec::new(),
            });
            continue;
        }
        if s.starts_with('[') {
            return Err(err(
                "plain `[table]` headers are outside the subset; use `[[table]]` rows".to_string(),
            ));
        }
        let Some((key, value_text)) = s.split_once('=') else {
            return Err(err("expected `key = value`".to_string()));
        };
        let key = key.trim();
        if key.contains('.') {
            return Err(err(format!("dotted key {key:?} is outside the subset")));
        }
        if !is_bare_key(key) {
            return Err(err(format!(
                "key {key:?} is not a bare key (quoted keys are outside the subset)"
            )));
        }
        let value = parse_value(value_text.trim()).map_err(err)?;
        let fields = match doc.rows.last_mut() {
            Some(row) => &mut row.fields,
            None => &mut doc.top,
        };
        if let Some(prev) = fields.iter().find(|f| f.key == key) {
            return Err(err(format!(
                "duplicate key {key:?} (first on line {})",
                prev.line
            )));
        }
        fields.push(TomlField {
            key: key.to_string(),
            value,
            line,
        });
    }
    Ok(doc)
}

/// After a value or header only whitespace or a comment may follow.
fn only_comment(tail: &str) -> Result<(), String> {
    let t = tail.trim_start();
    if t.is_empty() || t.starts_with('#') {
        Ok(())
    } else {
        Err(format!("unexpected text after the value: {t:?}"))
    }
}

fn parse_value(v: &str) -> Result<TomlValue, String> {
    let Some(first) = v.chars().next() else {
        return Err("missing value".to_string());
    };
    match first {
        '"' => {
            if v.starts_with("\"\"\"") {
                return Err("multi-line strings are outside the subset".to_string());
            }
            parse_basic_string(&v[1..])
        }
        '\'' => Err("literal strings are outside the subset; use a basic \"string\"".to_string()),
        '{' => Err("inline tables are outside the subset".to_string()),
        '[' => Err("arrays are outside the subset; use one `[[table]]` row per item".to_string()),
        _ => {
            let (token, tail) = match v.find(|c: char| c.is_whitespace() || c == '#') {
                Some(i) => (&v[..i], &v[i..]),
                None => (v, ""),
            };
            if matches!(token, "true" | "false") {
                return Err("booleans are outside the subset".to_string());
            }
            let n = parse_integer(token)?;
            only_comment(tail)?;
            Ok(TomlValue::Int(n))
        }
    }
}

/// `body` is the text after the opening quote.
fn parse_basic_string(body: &str) -> Result<TomlValue, String> {
    let mut out = String::new();
    let mut chars = body.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                only_comment(&body[i + 1..])?;
                return Ok(TomlValue::Str(out));
            }
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => {
                    return Err(format!(
                        "escape \\{other} is outside the subset (only \\\" and \\\\)"
                    ));
                }
                None => return Err("unterminated string".to_string()),
            },
            c if (c as u32) < 0x20 && c != '\t' => {
                return Err(format!("control character U+{:04X} in a string", c as u32));
            }
            c => out.push(c),
        }
    }
    Err("unterminated string".to_string())
}

fn parse_integer(token: &str) -> Result<i64, String> {
    if token.contains(['.', 'e', 'E']) && token.chars().any(|c| c.is_ascii_digit()) {
        return Err(format!("{token:?}: floats are outside the subset"));
    }
    if token.contains([':', 'T', 'Z']) || token.matches('-').count() > 1 {
        return Err(format!("{token:?}: dates and times are outside the subset"));
    }
    let digits = token.strip_prefix('-').unwrap_or(token);
    let valid = !digits.is_empty()
        && digits.starts_with(|c: char| c.is_ascii_digit())
        && digits.ends_with(|c: char| c.is_ascii_digit())
        && digits.chars().all(|c| c.is_ascii_digit() || c == '_')
        && !digits.contains("__");
    if !valid {
        return Err(format!("{token:?} is not a string or a decimal integer"));
    }
    let plain: String = digits.chars().filter(|c| *c != '_').collect();
    if plain.len() > 1 && plain.starts_with('0') {
        return Err(format!("{token:?}: leading zeros are outside the subset"));
    }
    let signed = if token.starts_with('-') {
        format!("-{plain}")
    } else {
        plain
    };
    signed
        .parse::<i64>()
        .map_err(|e| format!("{token:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused_at(text: &str) -> (usize, String) {
        match parse_subset(text) {
            Ok(doc) => panic!("accepted: {doc:?}"),
            Err(e) => (e.line, e.reason),
        }
    }

    #[test]
    fn test_toml_subset_accepts_tables_strings_integers_comments() {
        let text = "# header comment\nschema = \"fmine.sources.v1\"\n\n[[source]]\nfile = \"a \\\"b\\\" \\\\ c\" # trailing\nbytes = 43_668\n\n[[source]]\nfile = \"d\"\nbytes = -7\n[[subject]]\nname = \"Loulo-Gounkoto\"\n";
        let doc = parse_subset(text).expect("subset");
        assert_eq!(
            doc.top_get("schema").map(|f| &f.value),
            Some(&TomlValue::Str("fmine.sources.v1".to_string()))
        );
        let sources: Vec<&TomlRow> = doc.rows_of("source").collect();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].line, 4);
        assert_eq!(
            sources[0].get("file").map(|f| &f.value),
            Some(&TomlValue::Str("a \"b\" \\ c".to_string()))
        );
        assert_eq!(sources[0].get("bytes").map(|f| &f.value), Some(&TomlValue::Int(43_668)));
        assert_eq!(sources[1].get("bytes").map(|f| (&f.value, f.line)), Some((&TomlValue::Int(-7), 10)));
        assert_eq!(doc.rows_of("subject").count(), 1);
        // Row order is file order across tables.
        let order: Vec<&str> = doc.rows.iter().map(|r| r.table.as_str()).collect();
        assert_eq!(order, ["source", "source", "subject"]);
    }

    #[test]
    fn test_toml_subset_refuses_inline_table_with_line() {
        let (line, why) = refused_at("[[a]]\nx = 1\ny = { z = 1 }\n");
        assert_eq!(line, 3);
        assert!(why.contains("inline tables"), "{why}");
    }

    #[test]
    fn test_toml_subset_refuses_array_with_line() {
        let (line, why) = refused_at("[[a]]\nchecks = [\"A1\", \"B2\"]\n");
        assert_eq!(line, 2);
        assert!(why.contains("arrays"), "{why}");
    }

    #[test]
    fn test_toml_subset_refuses_plain_table_with_line() {
        let (line, why) = refused_at("x = 1\n\n[profile]\n");
        assert_eq!(line, 3);
        assert!(why.contains("plain"), "{why}");
        assert_eq!(refused_at("a.b = 1\n").0, 1);
        assert_eq!(refused_at("\"q\" = 1\n").0, 1);
        assert_eq!(refused_at("[[a.b]]\n").0, 1);
    }

    #[test]
    fn test_toml_subset_refuses_float_and_bool() {
        assert!(refused_at("x = 1.5\n").1.contains("floats"));
        assert!(refused_at("x = 1e3\n").1.contains("floats"));
        assert!(refused_at("x = true\n").1.contains("booleans"));
        assert!(refused_at("x = 2026-11-09\n").1.contains("dates"));
        assert!(refused_at("x = 2026-11-09T11:00:00Z\n").1.contains("dates"));
        assert!(refused_at("x = 012\n").1.contains("leading zeros"));
        assert!(refused_at("x = 1__0\n").1.contains("decimal integer"));
        assert!(refused_at("x = _1\n").1.contains("decimal integer"));
        assert!(refused_at("x = 'lit'\n").1.contains("literal"));
        assert!(refused_at("x = \"\"\"m\"\"\"\n").1.contains("multi-line"));
        assert!(refused_at("x = \"a\\nb\"\n").1.contains("escape"));
        assert!(refused_at("x = \"open\n").1.contains("unterminated"));
        assert!(refused_at("x = \"v\" junk\n").1.contains("unexpected text"));
        assert!(refused_at("x = 99999999999999999999\n").1.contains("too large"));
    }

    #[test]
    fn test_toml_subset_refuses_duplicate_key() {
        let (line, why) = refused_at("[[a]]\nk = 1\nk = 2\n");
        assert_eq!(line, 3);
        assert!(why.contains("first on line 2"), "{why}");
        // The same key in two rows is fine.
        assert!(parse_subset("[[a]]\nk = 1\n[[a]]\nk = 2\n").is_ok());
        assert_eq!(refused_at("k = 1\nk = 1\n").0, 2);
    }
}
