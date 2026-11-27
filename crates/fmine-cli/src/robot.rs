//! `fmine robot schema` (`BD-116`, plan §30.2, `registries/verbs.toml`).
//!
//! Emits deterministic JSON Schema (Draft 2020-12) documents for `FMT-ENV-1` envelopes, `Refusal`
//! objects (populated live from `fmine_evidence::CATALOG`), canonical `Claim` objects,
//! `DecisionCard` objects, and the compiled registered verb catalog (`verb_catalog::VERB_CATALOG`,
//! generated from `registries/verbs.toml` and verified byte-for-byte by `xtask check-verbs`).

use std::path::Path;

use fmine_evidence::CATALOG;

use crate::verb_catalog::VERB_CATALOG;
use crate::{Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// CLI usage summary reported in `robot-docs` and usage errors.
pub const USAGE: &str = "robot schema [all|envelope|refusal|claim|decision|verbs]";

/// Machine-readable documentation block embedded in `fmine robot-docs`.
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.robot-schema.v1\",",
    "\"draft\":\"https://json-schema.org/draft/2020-12/schema\",",
    "\"targets\":[\"all\",\"envelope\",\"refusal\",\"claim\",\"decision\",\"verbs\"],",
    "\"invariants\":[\"INV-002 flags are pinned const:false in envelope and decision schemas\",",
    "\"refusal code enum is generated from fmine_evidence::CATALOG\",",
    "\"verbs enum is compiled from registries/verbs.toml via verb_catalog::VERB_CATALOG\"]}"
);

/// Selected schema target for `fmine robot schema`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    All,
    Envelope,
    Refusal,
    Claim,
    Decision,
    Verbs,
}

impl Target {
    /// Stable lowercase identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Envelope => "envelope",
            Self::Refusal => "refusal",
            Self::Claim => "claim",
            Self::Decision => "decision",
            Self::Verbs => "verbs",
        }
    }
}

/// Parsed `fmine robot schema` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub target: Target,
}

/// Parse `robot schema [target]` after global options (`--json`, `--format`, `--workspace`) are stripped.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("robot")
        || words.get(1).map(String::as_str) != Some("schema")
    {
        return Err(format!("expected 'fmine {USAGE}'"));
    }
    let rest = &words[2..];
    let target = match rest {
        [] => Target::All,
        [word] => parse_target(word)?,
        [flag, value] if flag == "--target" => parse_target(value)?,
        _ => return Err(format!("expected 'fmine {USAGE}'")),
    };
    Ok(Command { target })
}

fn parse_target(raw: &str) -> Result<Target, String> {
    match raw {
        "all" => Ok(Target::All),
        "envelope" => Ok(Target::Envelope),
        "refusal" => Ok(Target::Refusal),
        "claim" => Ok(Target::Claim),
        "decision" => Ok(Target::Decision),
        "verbs" => Ok(Target::Verbs),
        other => Err(format!(
            "unknown robot schema target '{other}' (expected all|envelope|refusal|claim|decision|verbs)"
        )),
    }
}

/// Execute `fmine robot schema`.
pub fn execute(cmd: &Command, _workspace_root: &Path) -> Envelope {
    let refusal_codes_json = CATALOG
        .iter()
        .map(|spec| format!("\"{}\"", escape_json(spec.code)))
        .collect::<Vec<_>>()
        .join(",");

    let refusal_schema = format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":\"urn:fmine:schema:refusal:v1\",\"title\":\"Refusal\",\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"code\",\"exit_code\",\"what\",\"fix\"],\"properties\":{{\"code\":{{\"type\":\"string\",\"enum\":[{refusal_codes_json}]}},\"exit_code\":{{\"type\":\"integer\",\"minimum\":1,\"maximum\":9}},\"what\":{{\"type\":\"string\",\"minLength\":1}},\"fix\":{{\"type\":\"string\",\"minLength\":1}}}}}}"
    );

    let flags_schema = concat!(
        "{\"type\":\"object\",\"additionalProperties\":false,",
        "\"required\":[\"reserve_disclosure_claim\",\"execution_authorized\",\"geotechnical_signoff\",\"regulatory_compliance_claim\"],",
        "\"properties\":{",
        "\"reserve_disclosure_claim\":{\"const\":false},",
        "\"execution_authorized\":{\"const\":false},",
        "\"geotechnical_signoff\":{\"const\":false},",
        "\"regulatory_compliance_claim\":{\"const\":false}}}"
    );

    let envelope_schema = format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":\"urn:fmine:schema:fmt-env-1\",\"title\":\"FMT-ENV-1 Envelope\",\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"schema_version\",\"verb\",\"status\",\"exit_code\",\"anchor_id\",\"flags\",\"certificate\",\"warnings\",\"next_actions\",\"payload\",\"refusal\"],\"properties\":{{\"schema_version\":{{\"const\":\"fmine.env.v1\"}},\"verb\":{{\"type\":\"string\",\"minLength\":1}},\"status\":{{\"type\":\"string\",\"enum\":[\"ok\",\"refused\",\"usage_error\"]}},\"exit_code\":{{\"type\":\"integer\",\"minimum\":0,\"maximum\":9}},\"anchor_id\":{{\"type\":[\"string\",\"null\"]}},\"flags\":{flags_schema},\"certificate\":{{\"type\":[\"string\",\"null\"]}},\"warnings\":{{\"type\":\"array\",\"items\":{{\"type\":\"string\"}}}},\"next_actions\":{{\"type\":\"array\",\"minItems\":1,\"items\":{{\"type\":\"object\",\"additionalProperties\":false,\"required\":[\"command\",\"why\"],\"properties\":{{\"command\":{{\"type\":\"string\",\"minLength\":1}},\"why\":{{\"type\":\"string\",\"minLength\":1}}}}}}}},\"no_claim\":{{\"type\":\"string\"}},\"payload\":{{}},\"refusal\":{{\"oneOf\":[{{\"type\":\"null\"}},{refusal_schema}]}}}}}}"
    );

    let claim_schema = concat!(
        "{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",",
        "\"$id\":\"urn:fmine:schema:claim:v1\",\"title\":\"Claim\",\"type\":\"object\",",
        "\"required\":[\"claim_id\",\"evidence_class\",\"calibration\",\"uu_budget\",\"sources\"],",
        "\"properties\":{",
        "\"claim_id\":{\"type\":\"string\",\"pattern\":\"^clm_[0-9a-f]{16,}$\"},",
        "\"value\":{\"type\":[\"string\",\"number\",\"object\"]},",
        "\"unit\":{\"type\":\"string\"},",
        "\"support\":{\"type\":\"string\"},",
        "\"evidence_class\":{\"type\":\"string\",\"enum\":[\"measured\",\"derived\",\"modeled\",\"synthetic\",\"assumption\",\"refused\"]},",
        "\"calibration\":{\"type\":\"string\",\"enum\":[\"calibrated\",\"uncalibrated\",\"failed\",\"not_applicable\"]},",
        "\"uu_budget\":{\"type\":\"object\"},",
        "\"sources\":{\"type\":\"array\",\"minItems\":1,\"items\":{\"type\":\"object\",\"required\":[\"artifact_path\",\"artifact_sha256\",\"locator\"],\"properties\":{\"artifact_path\":{\"type\":\"string\"},\"artifact_sha256\":{\"type\":\"string\",\"pattern\":\"^[0-9a-f]{64}$\"},\"locator\":{\"type\":\"string\"}}}}}}"
    );

    let decision_schema = format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":\"urn:fmine:schema:decision-card:v1\",\"title\":\"DecisionCard\",\"type\":\"object\",\"required\":[\"schema\",\"alternatives\",\"uu_budget\",\"flip_margins\",\"flags\"],\"properties\":{{\"schema\":{{\"type\":\"string\"}},\"alternatives\":{{\"type\":\"array\",\"minItems\":1}},\"uu_budget\":{{\"type\":\"object\"}},\"flip_margins\":{{\"type\":\"array\"}},\"flags\":{flags_schema}}}}}"
    );

    let verbs_json = VERB_CATALOG
        .iter()
        .map(|spec| format!("\"{}\"", escape_json(spec.path)))
        .collect::<Vec<_>>()
        .join(",");
    let verbs_schema = format!(
        "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":\"urn:fmine:schema:verbs:v1\",\"title\":\"Registered CLI Verbs\",\"type\":\"string\",\"enum\":[{verbs_json}]}}"
    );

    let payload = match cmd.target {
        Target::Envelope => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"envelope\",\"document\":{envelope_schema}}}"
        ),
        Target::Refusal => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"refusal\",\"document\":{refusal_schema}}}"
        ),
        Target::Claim => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"claim\",\"document\":{claim_schema}}}"
        ),
        Target::Decision => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"decision\",\"document\":{decision_schema}}}"
        ),
        Target::Verbs => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"verbs\",\"document\":{verbs_schema}}}"
        ),
        Target::All => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"all\",\"documents\":{{\"envelope\":{envelope_schema},\"refusal\":{refusal_schema},\"claim\":{claim_schema},\"decision\":{decision_schema},\"verbs\":{verbs_schema}}}}}"
        ),
    };

    Envelope::ok(
        "robot schema",
        payload,
        vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use fmine_evidence::{Refusal, RefusalCode};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[derive(Debug, Clone, PartialEq)]
    enum JsonVal {
        Null,
        Bool(bool),
        Int(i64),
        Float(f64),
        Str(String),
        Arr(Vec<JsonVal>),
        Obj(BTreeMap<String, JsonVal>),
    }

    impl JsonVal {
        fn as_obj(&self) -> Option<&BTreeMap<String, JsonVal>> {
            match self {
                Self::Obj(m) => Some(m),
                _ => None,
            }
        }
        fn as_arr(&self) -> Option<&[JsonVal]> {
            match self {
                Self::Arr(v) => Some(v),
                _ => None,
            }
        }
        fn as_str(&self) -> Option<&str> {
            match self {
                Self::Str(s) => Some(s),
                _ => None,
            }
        }
        fn as_i64(&self) -> Option<i64> {
            match self {
                Self::Int(n) => Some(*n),
                _ => None,
            }
        }
        fn get(&self, key: &str) -> Option<&JsonVal> {
            self.as_obj()?.get(key)
        }
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl<'a> Parser<'a> {
        fn parse(input: &'a str) -> Result<JsonVal, String> {
            let mut p = Self {
                bytes: input.as_bytes(),
                pos: 0,
            };
            let v = p.value()?;
            p.skip_ws();
            if p.pos != p.bytes.len() {
                return Err(format!("trailing bytes at {}", p.pos));
            }
            Ok(v)
        }

        fn skip_ws(&mut self) {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
        }

        fn peek(&mut self) -> Option<u8> {
            self.skip_ws();
            self.bytes.get(self.pos).copied()
        }

        fn value(&mut self) -> Result<JsonVal, String> {
            match self.peek() {
                Some(b'{') => self.object(),
                Some(b'[') => self.array(),
                Some(b'"') => Ok(JsonVal::Str(self.string()?)),
                Some(b't') => {
                    self.expect_lit(b"true")?;
                    Ok(JsonVal::Bool(true))
                }
                Some(b'f') => {
                    self.expect_lit(b"false")?;
                    Ok(JsonVal::Bool(false))
                }
                Some(b'n') => {
                    self.expect_lit(b"null")?;
                    Ok(JsonVal::Null)
                }
                Some(b'-' | b'0'..=b'9') => self.number(),
                other => Err(format!("unexpected token {other:?} at {}", self.pos)),
            }
        }

        fn expect_lit(&mut self, lit: &[u8]) -> Result<(), String> {
            self.skip_ws();
            if self.bytes.get(self.pos..self.pos + lit.len()) == Some(lit) {
                self.pos += lit.len();
                Ok(())
            } else {
                Err(format!("expected {:?} at {}", lit, self.pos))
            }
        }

        fn string(&mut self) -> Result<String, String> {
            self.skip_ws();
            if self.bytes.get(self.pos) != Some(&b'"') {
                return Err(format!("expected '\"' at {}", self.pos));
            }
            self.pos += 1;
            let mut out = String::new();
            while self.pos < self.bytes.len() {
                let b = self.bytes[self.pos];
                self.pos += 1;
                match b {
                    b'"' => return Ok(out),
                    b'\\' => {
                        let esc = *self
                            .bytes
                            .get(self.pos)
                            .ok_or_else(|| "truncated escape".to_string())?;
                        self.pos += 1;
                        match esc {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{0008}'),
                            b'f' => out.push('\u{000c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let hex = std::str::from_utf8(
                                    self.bytes
                                        .get(self.pos..self.pos + 4)
                                        .ok_or_else(|| "truncated \\u escape".to_string())?,
                                )
                                .map_err(|e| e.to_string())?;
                                self.pos += 4;
                                let cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                                if let Some(ch) = char::from_u32(cp) {
                                    out.push(ch);
                                }
                            }
                            other => return Err(format!("bad escape \\{other}")),
                        }
                    }
                    _ => {
                        // Read full UTF-8 char starting at pos - 1.
                        let start = self.pos - 1;
                        let tail =
                            std::str::from_utf8(&self.bytes[start..]).map_err(|e| e.to_string())?;
                        let ch = tail.chars().next().ok_or_else(|| "eof".to_string())?;
                        out.push(ch);
                        self.pos = start + ch.len_utf8();
                    }
                }
            }
            Err("unterminated string".to_string())
        }

        fn number(&mut self) -> Result<JsonVal, String> {
            self.skip_ws();
            let start = self.pos;
            while self.pos < self.bytes.len()
                && matches!(
                    self.bytes[self.pos],
                    b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                )
            {
                self.pos += 1;
            }
            let s = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|e| e.to_string())?;
            if let Ok(i) = s.parse::<i64>() {
                Ok(JsonVal::Int(i))
            } else {
                let f = s.parse::<f64>().map_err(|e| e.to_string())?;
                Ok(JsonVal::Float(f))
            }
        }

        fn array(&mut self) -> Result<JsonVal, String> {
            self.pos += 1;
            let mut items = Vec::new();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(JsonVal::Arr(items));
            }
            loop {
                items.push(self.value()?);
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b']') => {
                        self.pos += 1;
                        return Ok(JsonVal::Arr(items));
                    }
                    other => return Err(format!("expected ',' or ']' got {other:?}")),
                }
            }
        }

        fn object(&mut self) -> Result<JsonVal, String> {
            self.pos += 1;
            let mut map = BTreeMap::new();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(JsonVal::Obj(map));
            }
            loop {
                let k = self.string()?;
                if self.peek() != Some(b':') {
                    return Err(format!("expected ':' after {k:?}"));
                }
                self.pos += 1;
                let v = self.value()?;
                map.insert(k, v);
                match self.peek() {
                    Some(b',') => self.pos += 1,
                    Some(b'}') => {
                        self.pos += 1;
                        return Ok(JsonVal::Obj(map));
                    }
                    other => return Err(format!("expected ',' or '}}' got {other:?}")),
                }
            }
        }
    }

    fn matches_type(val: &JsonVal, ty: &str) -> bool {
        match ty {
            "null" => matches!(val, JsonVal::Null),
            "boolean" => matches!(val, JsonVal::Bool(_)),
            "integer" => matches!(val, JsonVal::Int(_)),
            "number" => matches!(val, JsonVal::Int(_) | JsonVal::Float(_)),
            "string" => matches!(val, JsonVal::Str(_)),
            "array" => matches!(val, JsonVal::Arr(_)),
            "object" => matches!(val, JsonVal::Obj(_)),
            _ => false,
        }
    }

    fn validate_schema(schema: &JsonVal, instance: &JsonVal, path: &str) -> Result<(), String> {
        let Some(s) = schema.as_obj() else {
            return Err(format!("{path}: schema is not an object"));
        };
        if let Some(c) = s.get("const")
            && instance != c
        {
            return Err(format!("{path}: expected const {c:?}, got {instance:?}"));
        }
        if let Some(en) = s.get("enum").and_then(JsonVal::as_arr)
            && !en.iter().any(|v| v == instance)
        {
            return Err(format!("{path}: value {instance:?} not in enum"));
        }
        if let Some(ty) = s.get("type") {
            let ok = match ty {
                JsonVal::Str(t) => matches_type(instance, t),
                JsonVal::Arr(ts) => ts
                    .iter()
                    .filter_map(JsonVal::as_str)
                    .any(|t| matches_type(instance, t)),
                _ => false,
            };
            if !ok {
                return Err(format!(
                    "{path}: type mismatch for {ty:?}, got {instance:?}"
                ));
            }
        }
        if let Some(min) = s.get("minimum").and_then(JsonVal::as_i64)
            && let Some(n) = instance.as_i64()
            && n < min
        {
            return Err(format!("{path}: {n} < minimum {min}"));
        }
        if let Some(max) = s.get("maximum").and_then(JsonVal::as_i64)
            && let Some(n) = instance.as_i64()
            && n > max
        {
            return Err(format!("{path}: {n} > maximum {max}"));
        }
        if let Some(min_len) = s.get("minLength").and_then(JsonVal::as_i64)
            && let Some(st) = instance.as_str()
            && (st.len() as i64) < min_len
        {
            return Err(format!("{path}: string shorter than minLength {min_len}"));
        }
        if let Some(pattern) = s.get("pattern").and_then(JsonVal::as_str)
            && let Some(st) = instance.as_str()
        {
            let ok = match pattern {
                "^clm_[0-9a-f]{16,}$" => st.strip_prefix("clm_").is_some_and(|hex| {
                    hex.len() >= 16
                        && hex
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                }),
                "^[0-9a-f]{64}$" => {
                    st.len() == 64
                        && st
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                }
                _ => true,
            };
            if !ok {
                return Err(format!("{path}: string {st:?} failed pattern {pattern:?}"));
            }
        }
        if let Some(one_of) = s.get("oneOf").and_then(JsonVal::as_arr) {
            let matches = one_of
                .iter()
                .filter(|sub| validate_schema(sub, instance, path).is_ok())
                .count();
            if matches != 1 {
                return Err(format!(
                    "{path}: expected exactly 1 oneOf match, got {matches}"
                ));
            }
        }
        if let Some(arr) = instance.as_arr() {
            if let Some(min_items) = s.get("minItems").and_then(JsonVal::as_i64)
                && (arr.len() as i64) < min_items
            {
                return Err(format!(
                    "{path}: array length {} < minItems {min_items}",
                    arr.len()
                ));
            }
            if let Some(item_schema) = s.get("items") {
                for (idx, item) in arr.iter().enumerate() {
                    validate_schema(item_schema, item, &format!("{path}[{idx}]"))?;
                }
            }
        }
        if let Some(obj) = instance.as_obj() {
            if let Some(req) = s.get("required").and_then(JsonVal::as_arr) {
                for key in req.iter().filter_map(JsonVal::as_str) {
                    if !obj.contains_key(key) {
                        return Err(format!("{path}: missing required key {key:?}"));
                    }
                }
            }
            let props = s.get("properties").and_then(JsonVal::as_obj);
            let disallow_extra =
                matches!(s.get("additionalProperties"), Some(JsonVal::Bool(false)));
            for (k, v) in obj {
                if let Some(prop_schema) = props.and_then(|p| p.get(k)) {
                    validate_schema(prop_schema, v, &format!("{path}.{k}"))?;
                } else if disallow_extra {
                    return Err(format!("{path}: unexpected additional property {k:?}"));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_robot_schema_emits_all_targets_pins_inv_002_and_includes_live_catalogs() {
        let root = workspace_root();
        let env = execute(
            &Command {
                target: Target::All,
            },
            &root,
        );
        assert_eq!(env.exit_code, 0);
        assert_eq!(env.verb, "robot schema");
        let text = env.to_json();
        assert!(text.contains("\"schema\":\"fmine.robot-schema.v1\""));
        assert!(text.contains("\"reserve_disclosure_claim\":{\"const\":false}"));
        assert!(text.contains("\"execution_authorized\":{\"const\":false}"));
        assert!(text.contains("\"geotechnical_signoff\":{\"const\":false}"));
        assert!(text.contains("\"regulatory_compliance_claim\":{\"const\":false}"));
        for spec in CATALOG {
            assert!(
                text.contains(spec.code),
                "refusal schema missing catalog code {}",
                spec.code
            );
        }
        assert!(text.contains("\"robot schema\""));
        assert!(text.contains("\"repair plan\""));
        assert!(text.contains("\"repair apply\""));

        // Individual targets parse cleanly and reject unknown targets.
        for name in ["all", "envelope", "refusal", "claim", "decision", "verbs"] {
            let cmd = parse(&["robot".into(), "schema".into(), name.into()]).expect("parse");
            assert_eq!(cmd.target.as_str(), name);
            let single = execute(&cmd, &root);
            assert_eq!(single.exit_code, 0);
        }
        assert!(parse(&["robot".into(), "schema".into(), "bogus".into()]).is_err());
    }

    #[test]
    fn test_robot_schemas_validate_all_cli_outputs() {
        let root = workspace_root();
        let all_env = execute(
            &Command {
                target: Target::All,
            },
            &root,
        );
        let all_json = Parser::parse(&all_env.to_json()).expect("parse robot schema all");
        let docs = all_json
            .get("payload")
            .and_then(|p| p.get("documents"))
            .expect("documents object");
        let envelope_schema = docs.get("envelope").expect("envelope schema");
        let refusal_schema = docs.get("refusal").expect("refusal schema");
        let claim_schema = docs.get("claim").expect("claim schema");
        let decision_schema = docs.get("decision").expect("decision schema");
        let verbs_schema = docs.get("verbs").expect("verbs schema");

        // 1. Validate live CLI command envelopes (ok, refused, usage_error) against envelope schema.
        let sample_invocations: &[&[&str]] = &[
            &["robot-docs"],
            &["robot", "schema", "all"],
            &["doctor"],
            &["assure", "public", "--synthetic"],
            &[
                "plan",
                "cutoff",
                "--input",
                "tests/planning/lane_cutoff.json",
            ],
            &[
                "plan",
                "cutoff",
                "--input",
                "/nonexistent/fmine/lane-cutoff.json",
            ],
            &["forecast", "record", "check"],
            &[
                "plant",
                "recovery-audit",
                "--input",
                "/nonexistent/fmine/recovery-audit.toml",
            ],
            &[
                "repair",
                "plan",
                "--segment",
                "/nonexistent/fmine/seg.fmseg",
            ],
            &["totally-unknown-verb"],
        ];
        for argv in sample_invocations {
            let args: Vec<String> = argv.iter().map(|s| (*s).to_string()).collect();
            let (env, _) = crate::invoke(&args, &root);
            let parsed = Parser::parse(&env.to_json())
                .unwrap_or_else(|e| panic!("failed to parse JSON for {argv:?}: {e}"));
            validate_schema(envelope_schema, &parsed, &format!("envelope({argv:?})"))
                .unwrap_or_else(|e| panic!("schema validation failed for {argv:?}: {e}"));
        }

        // 2. Validate every registered refusal code against refusal schema.
        for code in RefusalCode::ALL {
            let r = Refusal::new(code, "synthetic detail");
            let raw = format!(
                "{{\"code\":\"{}\",\"exit_code\":{},\"what\":\"{}\",\"fix\":\"{}\"}}",
                escape_json(r.code),
                r.exit_code,
                escape_json(&r.what),
                escape_json(r.fix)
            );
            let parsed = Parser::parse(&raw).expect("parse refusal json");
            validate_schema(refusal_schema, &parsed, r.code).expect("refusal validates");
        }

        // 3. Validate every compiled verb in VERB_CATALOG against verbs schema (and reject unknown).
        assert_eq!(VERB_CATALOG.len(), 97);
        assert!(VERB_CATALOG.iter().any(|verb| verb.path == "drill desurvey"
            && verb.subsystem == "Corebox"
            && !verb.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|verb| verb.path == "plan blend"
            && verb.subsystem == "Pushback"
            && !verb.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|verb| verb.path == "plan cutoff"
            && verb.subsystem == "Pushback"
            && !verb.mutates_authority));
        for path in [
            "forecast journal investigate",
            "forecast journal resolve-investigation",
            "forecast journal rearm-investigation",
        ] {
            assert!(
                VERB_CATALOG.iter().any(|verb| verb.path == path
                    && verb.subsystem == "Plumb"
                    && verb.mutates_authority),
                "{path}"
            );
        }
        assert!(VERB_CATALOG.iter().any(|v| v.path == "screen target"
            && v.subsystem == "Lodestar"
            && !v.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|v| v.path == "check developer-npv"
            && v.subsystem == "Helm"
            && !v.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|v| v.path == "check drill-release"
            && v.subsystem == "Lodestar"
            && !v.mutates_authority));
        assert!(
            VERB_CATALOG.iter().any(|v| v.path == "decide hindcast"
                && v.subsystem == "Helm"
                && !v.mutates_authority)
        );
        assert!(
            VERB_CATALOG
                .iter()
                .any(|v| v.path == "forecast record check"
                    && v.subsystem == "Plumb"
                    && !v.mutates_authority)
        );
        assert!(VERB_CATALOG.iter().any(|v| v.path == "metrics waterfall"
            && v.subsystem == "Bullion"
            && !v.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|v| v.path == "plant reconcile-raw"
            && v.subsystem == "Crucible"
            && !v.mutates_authority));
        assert!(VERB_CATALOG.iter().any(|v| v.path == "plant recovery-audit"
            && v.subsystem == "Crucible"
            && !v.mutates_authority));
        assert!(
            VERB_CATALOG
                .iter()
                .any(|v| v.path == "forecast journal resolve-plant"
                    && v.subsystem == "Plumb"
                    && v.mutates_authority)
        );
        assert!(
            VERB_CATALOG
                .iter()
                .any(|v| v.path == "forecast journal explain-plant-change"
                    && v.subsystem == "Touchstone"
                    && !v.mutates_authority)
        );
        assert!(
            VERB_CATALOG.iter().any(|v| v.path == "recon run"
                && v.subsystem == "Touchstone"
                && !v.mutates_authority)
        );
        assert!(
            VERB_CATALOG
                .iter()
                .any(|v| v.path == "forecast spatial-monitor"
                    && v.subsystem == "Plumb"
                    && !v.mutates_authority)
        );
        assert!(
            VERB_CATALOG.iter().any(|v| v.path == "metrics roce"
                && v.subsystem == "Bullion"
                && !v.mutates_authority)
        );
        for spec in &VERB_CATALOG {
            validate_schema(
                verbs_schema,
                &JsonVal::Str(spec.path.to_string()),
                spec.path,
            )
            .expect("registered verb validates");
        }
        assert!(
            validate_schema(
                verbs_schema,
                &JsonVal::Str("unregistered verb".to_string()),
                "bogus"
            )
            .is_err()
        );

        // 4. Validate canonical Claim and DecisionCard instances against claim/decision schemas.
        let claim_sample = Parser::parse(
            r#"{"claim_id":"clm_0123456789abcdef0123456789abcdef","value":"100","unit":"oz","support":"Annual","evidence_class":"measured","calibration":"not_applicable","uu_budget":{},"sources":[{"artifact_path":"f.xlsx","artifact_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","locator":"S!A1"}]}"#,
        )
        .expect("parse claim");
        validate_schema(claim_schema, &claim_sample, "claim").expect("claim validates");

        let decision_sample = Parser::parse(
            r#"{"schema":"fmine.decision.v1","alternatives":[{"id":"a1"}],"uu_budget":{},"flip_margins":[],"flags":{"reserve_disclosure_claim":false,"execution_authorized":false,"geotechnical_signoff":false,"regulatory_compliance_claim":false}}"#,
        )
        .expect("parse decision");
        validate_schema(decision_schema, &decision_sample, "decision").expect("decision validates");

        // 5. Negative controls: INV-002 violation, extra envelope field, and unregistered refusal fail.
        let mut bad_inv002 = all_json.clone();
        let JsonVal::Obj(envelope) = &mut bad_inv002 else {
            panic!("expected envelope object")
        };
        let Some(JsonVal::Obj(flags)) = envelope.get_mut("flags") else {
            panic!("expected envelope flags")
        };
        let flag = flags
            .get_mut("execution_authorized")
            .expect("execution flag");
        assert_eq!(*flag, JsonVal::Bool(false));
        *flag = JsonVal::Bool(true);
        assert!(validate_schema(envelope_schema, &bad_inv002, "bad_inv002").is_err());

        let bogus_code = format!("{}-999", "FMINE-REF");
        let bad_refusal_json = format!(
            "{{\"code\":\"{bogus_code}\",\"exit_code\":3,\"what\":\"bad\",\"fix\":\"none\"}}"
        );
        let bad_refusal = Parser::parse(&bad_refusal_json).expect("parse bad refusal");
        assert!(validate_schema(refusal_schema, &bad_refusal, "bad_refusal").is_err());
    }
}
