//! `fmine robot schema` (`BD-116`, plan §30.2, `registries/verbs.toml`).
//!
//! Emits deterministic JSON Schema (Draft 2020-12) documents for `FMT-ENV-1` envelopes, `Refusal`
//! objects (populated live from `fmine_evidence::CATALOG`), canonical `Claim` objects,
//! `DecisionCard` objects, and the full registered verb catalog from `registries/verbs.toml`.

use std::path::Path;

use fmine_evidence::{CATALOG, Refusal, RefusalCode};
use fmine_registry::RegistryCatalog;

use crate::{Envelope, NextAction, ROBOT_DOCS_WHY, escape_json};

/// CLI usage summary reported in `robot-docs` and usage errors.
pub const USAGE: &str = "robot schema [all|envelope|refusal|claim|decision|verbs]";

/// Machine-readable documentation block embedded in `fmine robot-docs`.
pub const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.robot-schema.v1\",",
    "\"draft\":\"https://json-schema.org/draft/2020-12/schema\",",
    "\"targets\":[\"all\",\"envelope\",\"refusal\",\"claim\",\"decision\",\"verbs\"],",
    "\"invariants\":[\"INV-002 flags are pinned const:false in envelope and decision schemas\",",
    "\"refusal code enum is generated live from fmine_evidence::CATALOG\",",
    "\"verbs enum is loaded live from registries/verbs.toml\"]}"
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
pub fn execute(cmd: &Command, workspace_root: &Path) -> Envelope {
    let verbs_from_registry = match cmd.target {
        Target::All | Target::Verbs => match RegistryCatalog::load_from_root(workspace_root) {
            Ok(cat) => Some(cat.verb_names),
            Err(err) => {
                return Envelope::refused(
                    "robot schema",
                    Refusal::new(RefusalCode::InputMissingOrUnreadable506, err.to_string()),
                    vec![NextAction::new("fmine doctor", "diagnose registry health")],
                );
            }
        },
        _ => None,
    };

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

    let verbs_schema = verbs_from_registry.map(|verbs| {
        let verbs_json = verbs
            .iter()
            .map(|v| format!("\"{}\"", escape_json(v)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"$schema\":\"https://json-schema.org/draft/2020-12/schema\",\"$id\":\"urn:fmine:schema:verbs:v1\",\"title\":\"Registered CLI Verbs\",\"type\":\"string\",\"enum\":[{verbs_json}]}}"
        )
    });

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
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"verbs\",\"document\":{}}}",
            verbs_schema.unwrap_or_else(|| "null".to_string())
        ),
        Target::All => format!(
            "{{\"schema\":\"fmine.robot-schema.v1\",\"target\":\"all\",\"documents\":{{\"envelope\":{envelope_schema},\"refusal\":{refusal_schema},\"claim\":{claim_schema},\"decision\":{decision_schema},\"verbs\":{}}}}}",
            verbs_schema.unwrap_or_else(|| "null".to_string())
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
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
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
}
