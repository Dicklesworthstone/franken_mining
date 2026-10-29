//! FrankenMining CLI (`fmine`) implementation (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::path::{Path, PathBuf};

use fmine_evidence::{REFUSAL_CATALOG, Refusal};
use fmine_public::sources::issuer_profile;
use fmine_public::{run_public_assurance, run_synthetic_assurance};
use fmine_registry::RegistryCatalog;

/// Output serialization format (`BD-111`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Markdown,
}

/// Parsed CLI command (`BD-111`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// `fmine assure public --issuer <issuer>` (`BD-071`): the gate-eligible run over registered filings.
    AssurePublic { issuer: String },
    /// `fmine assure public --synthetic`: the known-answer run over synthetic workbooks (never gate-eligible).
    AssureSynthetic,
    /// `fmine doctor` (`BD-115`).
    Doctor,
    /// `fmine claim show <id>` (`BD-116`).
    ClaimShow { id: String },
    /// `fmine robot-docs` (`BD-117`).
    RobotDocs,
}

/// `FMT-ENV-1` structured CLI response envelope (`BD-112`, `BD-113`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub schema_version: &'static str,
    pub verb: String,
    pub status: &'static str,
    pub exit_code: u8,
    pub anchor_id: Option<String>,
    pub payload_json: String,
    pub refusal: Option<Refusal>,
}

impl Envelope {
    /// Constructs a successful `FMT-ENV-1` envelope (`exit_code = 0`).
    #[must_use]
    pub fn ok(verb: impl Into<String>, anchor_id: Option<String>, payload_json: String) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb: verb.into(),
            status: "ok",
            exit_code: 0,
            anchor_id,
            payload_json,
            refusal: None,
        }
    }

    /// Constructs a usage-error envelope (`exit_code = 2`, no refusal code).
    #[must_use]
    pub fn usage_error(verb: impl Into<String>, message: &str) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb: verb.into(),
            status: "usage_error",
            exit_code: 2,
            anchor_id: None,
            payload_json: format!("{{\"error\":\"{}\"}}", escape_json(message)),
            refusal: None,
        }
    }

    /// Constructs a refusal `FMT-ENV-1` envelope (`exit_code = refusal.exit_code`).
    #[must_use]
    pub fn refused(verb: impl Into<String>, refusal: Refusal) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb: verb.into(),
            status: "refused",
            exit_code: refusal.exit_code,
            anchor_id: None,
            payload_json: "null".to_string(),
            refusal: Some(refusal),
        }
    }

    /// Serializes this `FMT-ENV-1` envelope to canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let anchor_str = match &self.anchor_id {
            Some(a) => format!("\"{}\"", escape_json(a)),
            None => "null".to_string(),
        };
        let refusal_str = match &self.refusal {
            Some(r) => format!(
                "{{\"code\":\"{}\",\"exit_code\":{},\"what\":\"{}\",\"fix\":\"{}\"}}",
                escape_json(r.code),
                r.exit_code,
                escape_json(&r.what),
                escape_json(r.fix)
            ),
            None => "null".to_string(),
        };
        format!(
            "{{\"schema_version\":\"{}\",\"verb\":\"{}\",\"status\":\"{}\",\"exit_code\":{},\"anchor_id\":{},\"payload\":{},\"refusal\":{}}}",
            self.schema_version,
            escape_json(&self.verb),
            self.status,
            self.exit_code,
            anchor_str,
            self.payload_json,
            refusal_str
        )
    }
}

/// Parses command-line arguments into `(CliCommand, OutputFormat, PathBuf)`.
pub fn parse_args(
    args: &[String],
    default_root: &Path,
) -> Result<(CliCommand, OutputFormat, PathBuf), String> {
    let mut format = OutputFormat::Json;
    let mut workspace = default_root.to_path_buf();
    let mut positional = Vec::new();
    let mut synthetic = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --format")?;
                format = match val.as_str() {
                    "json" => OutputFormat::Json,
                    "markdown" | "md" => OutputFormat::Markdown,
                    other => {
                        return Err(format!(
                            "unsupported --format '{other}' (expected json|markdown)"
                        ));
                    }
                };
            }
            "--workspace" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --workspace")?;
                workspace = PathBuf::from(val);
            }
            "--issuer" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --issuer")?;
                positional.push(format!("issuer={val}"));
            }
            "--synthetic" => synthetic = true,
            arg => positional.push(arg.to_string()),
        }
        i += 1;
    }

    if positional.is_empty() {
        return Err(
            "missing command verb (try: assure public --issuer barrick | assure public --synthetic | doctor | robot-docs)"
                .to_string(),
        );
    }

    let cmd = match positional[0].as_str() {
        "assure" => {
            if positional.get(1).map(String::as_str) != Some("public") {
                return Err("expected 'fmine assure public --issuer <issuer>' or 'fmine assure public --synthetic'".to_string());
            }
            let issuer = positional.iter().find_map(|s| s.strip_prefix("issuer="));
            match (issuer, synthetic) {
                (Some(_), true) => return Err("--issuer and --synthetic are exclusive".to_string()),
                (None, true) => CliCommand::AssureSynthetic,
                (None, false) => {
                    return Err(
                        "missing --issuer <issuer> (registered: barrick) or --synthetic"
                            .to_string(),
                    );
                }
                (Some(name), false) => {
                    if issuer_profile(name).is_none() {
                        return Err(format!("unknown issuer '{name}' (registered: barrick)"));
                    }
                    CliCommand::AssurePublic {
                        issuer: name.to_string(),
                    }
                }
            }
        }
        "doctor" => CliCommand::Doctor,
        "claim" => {
            if positional.get(1).map(String::as_str) != Some("show") {
                return Err("expected 'fmine claim show <id>'".to_string());
            }
            let id = positional
                .get(2)
                .ok_or("missing <id> for 'fmine claim show <id>'")?
                .clone();
            CliCommand::ClaimShow { id }
        }
        "robot-docs" => CliCommand::RobotDocs,
        other => return Err(format!("unknown verb '{other}'")),
    };

    Ok((cmd, format, workspace))
}

/// Executes a parsed `CliCommand` against `workspace_root` and returns an `FMT-ENV-1` `Envelope`.
#[must_use]
pub fn execute_command(cmd: &CliCommand, workspace_root: &Path) -> Envelope {
    match cmd {
        CliCommand::AssurePublic { issuer } => {
            let Some(profile) = issuer_profile(issuer) else {
                return Envelope::usage_error(
                    "assure public",
                    &format!("unknown issuer '{issuer}' (registered: barrick)"),
                );
            };
            match run_public_assurance(profile, workspace_root) {
                Ok(report) => Envelope::ok(
                    "assure public",
                    Some(report.ledger_anchor.anchor_id().to_string()),
                    report.to_json(),
                ),
                Err(refusal) => Envelope::refused("assure public", refusal),
            }
        }
        CliCommand::AssureSynthetic => match run_synthetic_assurance(&[]) {
            Ok(report) => Envelope::ok(
                "assure public",
                Some(report.ledger_anchor.anchor_id().to_string()),
                report.to_json(),
            ),
            Err(refusal) => Envelope::refused("assure public", refusal),
        },
        CliCommand::Doctor => match RegistryCatalog::load_from_root(workspace_root) {
            Ok(cat) => {
                let barrick = &fmine_public::sources::BARRICK;
                let dir = workspace_root.join(barrick.corpus_dir);
                let present = barrick
                    .sources
                    .iter()
                    .filter(|s| dir.join(s.file_name).is_file())
                    .count();
                let payload = format!(
                    "{{\"registries_ok\":true,\"units_count\":{},\"crs_count\":{},\"refusals_count\":{},\"verbs_count\":{},\"barrick_sources_registered\":{},\"barrick_sources_present\":{}}}",
                    cat.unit_symbols.len(),
                    cat.crs_codes.len(),
                    cat.refusal_codes.len(),
                    cat.verb_names.len(),
                    barrick.sources.len(),
                    present
                );
                Envelope::ok("doctor", None, payload)
            }
            Err(e) => {
                Envelope::refused("doctor", Refusal::from_code("FMINE-REF-502", e.to_string()))
            }
        },
        CliCommand::ClaimShow { id } => Envelope::refused(
            "claim show",
            Refusal::from_code(
                "FMINE-REF-502",
                format!(
                    "no claim store exists in M0, so claim '{id}' cannot be read; claims are emitted only inside an \
                     'assure public' report"
                ),
            ),
        ),
        CliCommand::RobotDocs => {
            let refusals: Vec<String> = REFUSAL_CATALOG
                .iter()
                .map(|r| {
                    format!(
                        "{{\"code\":\"{}\",\"exit_code\":{},\"summary\":\"{}\"}}",
                        r.code,
                        r.exit_code,
                        escape_json(r.summary)
                    )
                })
                .collect();
            let payload = format!(
                "{{\"binary\":\"fmine\",\"envelope\":\"FMT-ENV-1\",\"verbs\":[\"assure public --issuer <issuer>\",\"assure public --synthetic\",\"doctor\",\"claim show <id>\",\"robot-docs\"],\"issuers\":[\"barrick\"],\"exit_codes\":{{\"0\":\"ok\",\"2\":\"usage error (unknown verb, flag or issuer)\"}},\"refusals\":[{}]}}",
                refusals.join(",")
            );
            Envelope::ok("robot-docs", None, payload)
        }
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn test_cli_assure_synthetic_end_to_end() {
        let root = workspace_root();
        let (cmd, fmt, ws) =
            parse_args(&args(&["assure", "public", "--synthetic"]), &root).expect("parse");
        assert_eq!(
            (cmd.clone(), fmt),
            (CliCommand::AssureSynthetic, OutputFormat::Json)
        );
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"));
        assert!(env.anchor_id.as_deref().unwrap_or("").starts_with("anc_"));
        let json = env.to_json();
        assert!(json.contains(
            "\"payload\":{\"issuer\":\"synthetic\",\"lane\":\"synthetic\",\"gate_eligible\":false"
        ));
        assert!(
            json.contains("\"totals\":{\"i\":21,\"ii\":1,\"iii\":1,\"iv\":1,\"v\":6,\"np\":5}")
        );
        assert!(
            json.contains(
                "\"next_actions\":[{\"command\":\"fmine assure public --issuer barrick\""
            )
        );
    }

    /// Without the registered workbooks the gate-eligible run refuses with exit 8 and names the files; it
    /// never substitutes stand-in content (BD-321).
    #[test]
    fn test_cli_assure_public_refuses_without_registered_files() {
        let empty = workspace_root()
            .join("crates")
            .join("fmine-cli")
            .join("no-such-workspace");
        let (cmd, _, ws) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &empty.display().to_string(),
            ]),
            &workspace_root(),
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (8, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-502\""));
        assert!(json.contains("Barrick_Q2_2026_AISC.xlsx"));
        assert!(json.contains("\"payload\":null"));
    }

    #[test]
    fn test_cli_usage_errors() {
        let root = workspace_root();
        for bad in [
            &["assure", "public", "--issuer", "newmont"][..],
            &["assure", "public"][..],
            &["assure", "public", "--issuer", "barrick", "--synthetic"][..],
            &["assure", "private"][..],
            &[][..],
        ] {
            assert!(
                parse_args(&args(bad), &root).is_err(),
                "{bad:?} must be a usage error"
            );
        }
        let env = execute_command(
            &CliCommand::AssurePublic {
                issuer: "newmont".to_string(),
            },
            &root,
        );
        assert_eq!(
            (env.exit_code, env.status, env.refusal.is_none()),
            (2, "usage_error", true)
        );
    }

    #[test]
    fn test_cli_claim_show_refuses_without_a_claim_store() {
        let env = execute_command(
            &CliCommand::ClaimShow {
                id: "clm_0001".to_string(),
            },
            &workspace_root(),
        );
        assert_eq!((env.exit_code, env.status), (8, "refused"));
        assert!(env.to_json().contains("no claim store exists in M0"));
    }

    /// Gate run on the real registered workbooks (public lane; the files are not in git). Run with
    /// `cargo test -p fmine-cli -- --ignored` where `corpus_of_input_docs/downloaded/mine_stats/` is present.
    #[test]
    #[ignore = "needs the 18 registered Barrick workbooks under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_cli_assure_public_barrick_real_files() {
        let root = workspace_root();
        let (cmd, _, ws) =
            parse_args(&args(&["assure", "public", "--issuer", "barrick"]), &root).expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        let json = env.to_json();
        assert!(json.contains("\"lane\":\"public\",\"gate_eligible\":true"));
        assert!(json.contains("\"aisc_tables\":34,\"instances\":2595"));
        assert!(
            json.contains(
                "\"totals\":{\"i\":2120,\"ii\":3,\"iii\":4,\"iv\":17,\"v\":391,\"np\":60}"
            )
        );
    }

    #[test]
    fn test_cli_doctor_and_robot_docs() {
        let root = workspace_root();
        let env_doc = execute_command(&CliCommand::Doctor, &root);
        assert_eq!(env_doc.exit_code, 0);
        assert!(env_doc.to_json().contains("\"registries_ok\":true"));

        assert!(
            env_doc
                .to_json()
                .contains("\"barrick_sources_registered\":18")
        );

        let env_rob = execute_command(&CliCommand::RobotDocs, &root);
        assert_eq!(env_rob.exit_code, 0);
        let rob = env_rob.to_json();
        assert!(rob.contains("\"FMT-ENV-1\""));
        assert!(rob.contains("{\"code\":\"FMINE-REF-502\",\"exit_code\":8,"));
    }
}
