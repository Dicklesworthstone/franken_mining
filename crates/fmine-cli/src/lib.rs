//! FrankenMining CLI (`fmine`) implementation (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::path::{Path, PathBuf};

use fmine_evidence::{REFUSAL_CATALOG, Refusal};
use fmine_public::sources::{
    is_issuer_name, is_through_bound, load_issuer_profile, registered_issuers, registry_path,
};
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
    /// `fmine assure public --issuer <issuer> [--through YYYYQn]` (`BD-071`): the gate-eligible run over the
    /// filings in `registries/sources/<issuer>.toml`, optionally only those at or before a quarter.
    AssurePublic {
        issuer: String,
        through: Option<String>,
    },
    /// `fmine assure public --synthetic`: the known-answer run over synthetic workbooks (never gate-eligible).
    AssureSynthetic,
    /// `fmine doctor` (`BD-115`).
    Doctor,
    /// `fmine claim show <id>` (`BD-116`).
    ClaimShow { id: String },
    /// `fmine robot-docs` (`BD-117`).
    RobotDocs,
}

/// The four `INV-002` flags, all false. They are a constant, with no field and no setter, so no code path can
/// set one (`BD-111`, `PO-ADIT-1`).
const FLAGS_JSON: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                          \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

/// A command worth running next, and why (`next_actions`). Every command parses with [`parse_args`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextAction {
    pub command: String,
    pub why: String,
}

impl NextAction {
    #[must_use]
    pub fn new(command: impl Into<String>, why: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            why: why.into(),
        }
    }
}

/// The envelope's fields, in print order, as `robot-docs` documents them.
const ENVELOPE_FIELDS: &str = "{\"schema_version\":\"fmine.env.v1\",\"verb\":\"the verb run\",\
\"status\":\"ok, refused or usage_error\",\"exit_code\":\"0, 2 (usage error) or the refusal's exit code\",\
\"anchor_id\":\"the report's ledger anchor, or null\",\"flags\":\"the four INV-002 flags, always false\",\
\"certificate\":\"the report's state root (hex), or null\",\"warnings\":\"array of strings\",\
\"next_actions\":\"array of {command, why}; every command parses\",\
\"no_claim\":\"report verbs only: what the report does not claim\",\
\"payload\":\"the verb's result, or null when refused\",\"refusal\":\"{code, exit_code, what, fix}, or null\"}";

const DOCTOR_WHY: &str = "which registered files are present, and whether every registry loads";
const ROBOT_DOCS_WHY: &str = "verbs, envelope fields and exit codes";

/// `FMT-ENV-1` structured CLI response envelope (`BD-111`, `BD-329.S3`). Field names are this crate's; the
/// renaming to FMT-ENV-1's (`v`, `event`, `node`, `anchor`, `data`) is open for the FMT-ENV-1 owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub schema_version: &'static str,
    pub verb: String,
    pub status: &'static str,
    pub exit_code: u8,
    pub anchor_id: Option<String>,
    /// The ledger state root (hex) of a report verb; `None` for every other verb.
    pub certificate: Option<String>,
    pub warnings: Vec<String>,
    pub next_actions: Vec<NextAction>,
    /// What a report verb does not claim; `None` for every other verb.
    pub no_claim: Option<&'static str>,
    pub payload_json: String,
    pub refusal: Option<Refusal>,
}

impl Envelope {
    fn new(verb: String, status: &'static str, exit_code: u8, payload_json: String) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb,
            status,
            exit_code,
            anchor_id: None,
            certificate: None,
            warnings: Vec::new(),
            next_actions: Vec::new(),
            no_claim: None,
            payload_json,
            refusal: None,
        }
    }

    /// A successful envelope for a verb that writes no report (`exit_code = 0`, no certificate).
    #[must_use]
    pub fn ok(
        verb: impl Into<String>,
        payload_json: String,
        next_actions: Vec<NextAction>,
    ) -> Self {
        Self {
            next_actions,
            ..Self::new(verb.into(), "ok", 0, payload_json)
        }
    }

    /// A successful report envelope: the anchor, the state root as `certificate`, and the no-claim line.
    #[must_use]
    pub fn report(
        verb: impl Into<String>,
        anchor_id: String,
        state_root_hex: String,
        payload_json: String,
        next_actions: Vec<NextAction>,
    ) -> Self {
        Self {
            anchor_id: Some(anchor_id),
            certificate: Some(state_root_hex),
            next_actions,
            no_claim: Some(fmine_public::NO_CLAIM),
            ..Self::new(verb.into(), "ok", 0, payload_json)
        }
    }

    /// A usage-error envelope (`exit_code = 2`, no refusal code), pointing to `fmine robot-docs`.
    #[must_use]
    pub fn usage_error(verb: impl Into<String>, message: &str) -> Self {
        Self {
            next_actions: vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
            ..Self::new(
                verb.into(),
                "usage_error",
                2,
                format!("{{\"error\":\"{}\"}}", escape_json(message)),
            )
        }
    }

    /// A refusal envelope (`exit_code = refusal.exit_code`).
    #[must_use]
    pub fn refused(
        verb: impl Into<String>,
        refusal: Refusal,
        next_actions: Vec<NextAction>,
    ) -> Self {
        let exit_code = refusal.exit_code;
        Self {
            next_actions,
            refusal: Some(refusal),
            ..Self::new(verb.into(), "refused", exit_code, "null".to_string())
        }
    }

    /// Serializes this `FMT-ENV-1` envelope to canonical JSON. After `anchor_id` come, in this order, `flags`,
    /// `certificate`, `warnings`, `next_actions`, `no_claim` (report verbs only), `payload` and `refusal`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let opt = |s: Option<&str>| {
            s.map_or_else(|| "null".to_string(), |v| format!("\"{}\"", escape_json(v)))
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
        let warnings: Vec<String> = self
            .warnings
            .iter()
            .map(|w| format!("\"{}\"", escape_json(w)))
            .collect();
        let actions: Vec<String> = self
            .next_actions
            .iter()
            .map(|a| {
                format!(
                    "{{\"command\":\"{}\",\"why\":\"{}\"}}",
                    escape_json(&a.command),
                    escape_json(&a.why)
                )
            })
            .collect();
        let no_claim = self
            .no_claim
            .map(|n| format!("\"no_claim\":\"{}\",", escape_json(n)))
            .unwrap_or_default();
        format!(
            "{{\"schema_version\":\"{}\",\"verb\":\"{}\",\"status\":\"{}\",\"exit_code\":{},\"anchor_id\":{},\
             \"flags\":{FLAGS_JSON},\"certificate\":{},\"warnings\":[{}],\"next_actions\":[{}],{no_claim}\
             \"payload\":{},\"refusal\":{}}}",
            self.schema_version,
            escape_json(&self.verb),
            self.status,
            self.exit_code,
            opt(self.anchor_id.as_deref()),
            opt(self.certificate.as_deref()),
            warnings.join(","),
            actions.join(","),
            self.payload_json,
            refusal_str
        )
    }
}

/// Runs `fmine` on `args` (without the program name) and returns the text for stdout and the exit code.
/// A parse error becomes a usage-error envelope (exit 2), so every invocation prints exactly one envelope.
#[must_use]
pub fn run(args: &[String], default_root: &Path) -> (String, u8) {
    let (env, format) = invoke(args, default_root);
    let text = match format {
        OutputFormat::Json => env.to_json(),
        OutputFormat::Markdown => format!(
            "# fmine `{}` ({})\n\n```json\n{}\n```",
            env.verb,
            env.status,
            env.to_json()
        ),
    };
    (text, env.exit_code)
}

/// The envelope for `args`, and the output format: [`run`] before printing. A parse error is a usage-error
/// envelope in JSON.
#[must_use]
pub fn invoke(args: &[String], default_root: &Path) -> (Envelope, OutputFormat) {
    match parse_args(args, default_root) {
        Ok((cmd, format, workspace)) => (execute_command(&cmd, &workspace), format),
        Err(message) => (
            Envelope::usage_error(verb_hint(args), &message),
            OutputFormat::Json,
        ),
    }
}

/// The verb a failed parse was aiming at, for its usage-error envelope: the verb the first word names, or
/// `unknown`.
fn verb_hint(args: &[String]) -> &'static str {
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--format" | "--workspace" | "--issuer" | "--through" => i += 2,
            flag if flag.starts_with("--") => i += 1,
            "assure" => return "assure public",
            "claim" => return "claim show",
            "doctor" => return "doctor",
            "robot-docs" => return "robot-docs",
            _ => return "unknown",
        }
    }
    "unknown"
}

/// Parses command-line arguments into `(CliCommand, OutputFormat, PathBuf)`.
pub fn parse_args(
    args: &[String],
    default_root: &Path,
) -> Result<(CliCommand, OutputFormat, PathBuf), String> {
    let mut format = OutputFormat::Json;
    let mut workspace = default_root.to_path_buf();
    let mut positional = Vec::new();
    let mut issuer: Option<String> = None;
    let mut synthetic = false;
    let mut through = None;

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
                issuer = Some(val.clone());
            }
            "--through" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --through")?;
                if !is_through_bound(val) {
                    return Err(format!("--through '{val}' is not a quarter YYYYQ1..YYYYQ4"));
                }
                through = Some(val.clone());
            }
            "--synthetic" => synthetic = true,
            flag if flag.starts_with("--") => return Err(format!("unknown flag '{flag}'")),
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

    // The words each verb takes, the verb included. A surplus word or a flag the verb does not take is a usage
    // error, never ignored, so every next action that parses means what it says.
    let words = match positional[0].as_str() {
        "assure" => 2,
        "claim" => 3,
        "doctor" | "robot-docs" => 1,
        other => return Err(format!("unknown verb '{other}'")),
    };
    if let Some(extra) = positional.get(words) {
        return Err(format!(
            "unexpected argument '{extra}' after '{}'",
            positional[..words].join(" ")
        ));
    }
    if positional[0] != "assure" && (issuer.is_some() || synthetic || through.is_some()) {
        return Err(
            "--issuer, --synthetic and --through apply to 'assure public' only".to_string(),
        );
    }

    let cmd = match positional[0].as_str() {
        "assure" => {
            if positional.get(1).map(String::as_str) != Some("public") {
                return Err("expected 'fmine assure public --issuer <issuer>' or 'fmine assure public --synthetic'".to_string());
            }
            let registered = || registered_issuers(&workspace).join(", ");
            match (issuer.as_deref(), synthetic) {
                (Some(_), true) => return Err("--issuer and --synthetic are exclusive".to_string()),
                (None, true) if through.is_some() => {
                    return Err("--through applies to --issuer runs only".to_string());
                }
                (None, true) => CliCommand::AssureSynthetic,
                (None, false) => {
                    return Err(format!(
                        "missing --issuer <issuer> (registered: {}) or --synthetic",
                        registered()
                    ));
                }
                (Some(name), false) => {
                    if !is_issuer_name(name) || !registry_path(&workspace, name).is_file() {
                        return Err(format!(
                            "unknown issuer '{name}': no registries/sources/{name}.toml under {} (registered: {})",
                            workspace.display(),
                            registered()
                        ));
                    }
                    CliCommand::AssurePublic {
                        issuer: name.to_string(),
                        through,
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

/// The envelope of a finished `assure public` report.
fn report_envelope(report: &fmine_public::PublicAssuranceReport) -> Envelope {
    Envelope::report(
        "assure public",
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        report.to_json(),
        report
            .next_actions()
            .into_iter()
            .map(|(command, why)| NextAction::new(command, why))
            .collect(),
    )
}

/// Executes a parsed `CliCommand` against `workspace_root` and returns an `FMT-ENV-1` `Envelope`.
#[must_use]
pub fn execute_command(cmd: &CliCommand, workspace_root: &Path) -> Envelope {
    let to_doctor = || vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match cmd {
        CliCommand::AssurePublic { issuer, through } => {
            if !is_issuer_name(issuer) || !registry_path(workspace_root, issuer).is_file() {
                return Envelope::usage_error(
                    "assure public",
                    &format!(
                        "unknown issuer '{issuer}' (registered: {})",
                        registered_issuers(workspace_root).join(", ")
                    ),
                );
            }
            let profile = match (load_issuer_profile(workspace_root, issuer), through) {
                (Ok(p), Some(bound)) => p.through(bound),
                (loaded, None) => loaded,
                (Err(e), Some(_)) => Err(e),
            };
            let profile = match profile {
                Ok(p) => p,
                Err(refusal) => return Envelope::refused("assure public", refusal, to_doctor()),
            };
            // BD-339 adds its reproduction kit to the refusal's next actions once it exists.
            match run_public_assurance(&profile, workspace_root) {
                Ok(report) => report_envelope(&report),
                Err(refusal) => Envelope::refused("assure public", refusal, to_doctor()),
            }
        }
        CliCommand::AssureSynthetic => match run_synthetic_assurance(&[]) {
            Ok(report) => report_envelope(&report),
            Err(refusal) => Envelope::refused("assure public", refusal, to_doctor()),
        },
        CliCommand::Doctor => match RegistryCatalog::load_from_root(workspace_root) {
            Ok(cat) => {
                // One entry per registries/sources/<issuer>.toml: registered and present counts, or the
                // registry's refusal (file and line) when it does not load.
                let issuers: Vec<String> = registered_issuers(workspace_root)
                    .iter()
                    .map(|name| match load_issuer_profile(workspace_root, name) {
                        Ok(p) => {
                            let dir = workspace_root.join(&p.corpus_dir);
                            let present = p
                                .sources
                                .iter()
                                .filter(|s| dir.join(&s.file_name).is_file())
                                .count();
                            format!(
                                "{{\"issuer\":\"{}\",\"registry_ok\":true,\"sources_registered\":{},\"sources_present\":{}}}",
                                escape_json(name),
                                p.sources.len(),
                                present
                            )
                        }
                        Err(r) => format!(
                            "{{\"issuer\":\"{}\",\"registry_ok\":false,\"error\":\"{}\"}}",
                            escape_json(name),
                            escape_json(&r.what)
                        ),
                    })
                    .collect();
                let payload = format!(
                    "{{\"registries_ok\":true,\"units_count\":{},\"crs_count\":{},\"refusals_count\":{},\"verbs_count\":{},\"issuers\":[{}]}}",
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
            Err(e) => Envelope::refused(
                "doctor",
                Refusal::from_code("FMINE-REF-502", e.to_string()),
                vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
            ),
        },
        CliCommand::ClaimShow { id } => {
            let report = registered_issuers(workspace_root).first().map_or_else(
                || "fmine assure public --synthetic".to_string(),
                |issuer| format!("fmine assure public --issuer {issuer}"),
            );
            Envelope::refused(
                "claim show",
                Refusal::from_code(
                    "FMINE-REF-502",
                    format!(
                        "no claim store exists in M0, so claim '{id}' cannot be read; claims are emitted only \
                         inside an 'assure public' report"
                    ),
                ),
                vec![NextAction::new(
                    report,
                    "claims exist only inside an assure public report",
                )],
            )
        }
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
            let issuers: Vec<String> = registered_issuers(workspace_root)
                .iter()
                .map(|i| format!("\"{}\"", escape_json(i)))
                .collect();
            let payload = format!(
                "{{\"binary\":\"fmine\",\"envelope\":\"FMT-ENV-1\",\"envelope_fields\":{ENVELOPE_FIELDS},\"verbs\":[\"assure public --issuer <issuer> [--through YYYYQn]\",\"assure public --synthetic\",\"doctor\",\"claim show <id>\",\"robot-docs\"],\"issuers\":[{}],\"issuer_registry\":\"registries/sources/<issuer>.toml\",\"exit_codes\":{{\"0\":\"ok\",\"2\":\"usage error (unknown verb, flag, issuer or --through bound); the envelope still prints\"}},\"refusals\":[{}]}}",
                issuers.join(","),
                refusals.join(",")
            );
            Envelope::ok(
                "robot-docs",
                payload,
                vec![NextAction::new("fmine doctor", DOCTOR_WHY)],
            )
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
            json.contains("\"totals\":{\"i\":30,\"ii\":1,\"iii\":1,\"iv\":2,\"v\":6,\"np\":5}")
        );
        // The next actions sit in the envelope, not the payload.
        assert_eq!(
            env.next_actions.first().map(|a| a.command.as_str()),
            Some("fmine assure public --issuer barrick")
        );
        assert!(
            json.contains(
                "\"next_actions\":[{\"command\":\"fmine assure public --issuer barrick\""
            )
        );
        assert!(!env.payload_json.contains("next_actions"));
    }

    /// Without the registered workbooks the gate-eligible run refuses with exit 8 and names the files; it
    /// never substitutes stand-in content (BD-321).
    ///
    /// The scratch workspace holds a copy of the registry and no corpus.
    #[test]
    fn test_cli_assure_public_refuses_without_registered_files() {
        let ws = std::env::temp_dir().join(format!("fmine-cli-no-corpus-{}", std::process::id()));
        let reg = registry_path(&ws, "barrick");
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        std::fs::copy(registry_path(&workspace_root(), "barrick"), &reg).expect("copy registry");
        let ws_arg = ws.display().to_string();
        let (cmd, _, ws_parsed) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--workspace",
                &ws_arg,
            ]),
            &workspace_root(),
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws_parsed);
        assert_eq!((env.exit_code, env.status), (8, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-502\""));
        assert!(json.contains("Barrick_Q2_2026_AISC.xlsx"));
        assert!(json.contains("\"payload\":null"));

        // --through narrows the refusal to the rows at or before the bound.
        let (cmd, _, _) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2024Q4",
                "--workspace",
                &ws_arg,
            ]),
            &workspace_root(),
        )
        .expect("parse");
        assert_eq!(
            cmd,
            CliCommand::AssurePublic {
                issuer: "barrick".to_string(),
                through: Some("2024Q4".to_string())
            }
        );
        let json = execute_command(&cmd, &ws_parsed).to_json();
        assert!(json.contains("Barrick_Q4_2024_AISC.xlsx"), "{json}");
        assert!(!json.contains("Barrick_Q1_2025"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// A workspace without the issuer's registry file makes the issuer unknown: a usage error, exit 2.
    #[test]
    fn test_cli_unknown_issuer_without_registry_is_usage_error() {
        let empty = workspace_root()
            .join("crates")
            .join("fmine-cli")
            .join("no-such-workspace");
        let err = parse_args(
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
        .expect_err("no registry there");
        assert!(err.contains("no registries/sources/barrick.toml"), "{err}");
    }

    #[test]
    fn test_cli_usage_errors() {
        let root = workspace_root();
        for bad in [
            &["assure", "public", "--issuer", "newmont"][..],
            &["assure", "public"][..],
            &["assure", "public", "--issuer", "barrick", "--synthetic"][..],
            &["assure", "private"][..],
            &[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026Q5",
            ][..],
            &[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026-Q2",
            ][..],
            &["assure", "public", "--issuer", "barrick", "--through"][..],
            &["assure", "public", "--synthetic", "--through", "2026Q2"][..],
            &[
                "assure",
                "public",
                "--issuer",
                "../registries/sources/barrick",
            ][..],
            &[][..],
            // Unknown flags, surplus words and flags a verb does not take (none is ignored).
            &["doctor", "--deep"][..],
            &["robot-docs", "extra"][..],
            &["assure", "public", "issuer=barrick"][..],
            &["assure", "public", "--synthetic", "now"][..],
            &["claim", "show", "clm_0001", "clm_0002"][..],
            &["doctor", "--issuer", "barrick"][..],
            &["robot-docs", "--synthetic"][..],
        ] {
            assert!(
                parse_args(&args(bad), &root).is_err(),
                "{bad:?} must be a usage error"
            );
            // What `fmine` prints: one envelope line on stdout, exit 2.
            let (text, exit) = run(&args(bad), &root);
            assert_eq!(exit, 2, "{bad:?}");
            assert!(!text.contains('\n'), "{bad:?}: {text}");
            assert!(
                text.starts_with("{\"schema_version\":\"fmine.env.v1\",\"verb\":\""),
                "{bad:?}: {text}"
            );
            assert!(
                text.contains("\"status\":\"usage_error\",\"exit_code\":2,"),
                "{bad:?}: {text}"
            );
        }
        let env = execute_command(
            &CliCommand::AssurePublic {
                issuer: "newmont".to_string(),
                through: None,
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
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn test_cli_assure_public_barrick_real_files() {
        let root = workspace_root();
        let (cmd, _, ws) = parse_args(
            &args(&[
                "assure",
                "public",
                "--issuer",
                "barrick",
                "--through",
                "2026Q2",
            ]),
            &root,
        )
        .expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        let json = env.to_json();
        assert!(json.contains("\"lane\":\"public\",\"gate_eligible\":true"));
        assert!(json.contains("\"through\":\"2026Q2\""));
        // Regression pin of this implementation's output (MOR_CASE E822, E826), not an independent value.
        assert!(json.contains(
            "\"state_root\":\"be22f27186a4b923e366594c8e5376c4a0d3bb2fd3df23766417e03658adba82\""
        ));
        assert!(json.contains("\"aisc_tables\":34,\"instances\":2595"));
        assert!(json.contains(
            // BD-331.S3 (census E826): 260 stated-share fits move v → i and 65 misses v → iv.
            "\"totals\":{\"i\":2380,\"ii\":3,\"iii\":4,\"iv\":82,\"v\":66,\"np\":60}"
        ));
        // The public-lane envelope: the root as certificate, the no-claim line, and its two next actions.
        assert_eq!(
            env.certificate.as_deref(),
            Some("be22f27186a4b923e366594c8e5376c4a0d3bb2fd3df23766417e03658adba82")
        );
        assert_eq!(env.no_claim, Some(fmine_public::NO_CLAIM));
        let commands: Vec<&str> = env
            .next_actions
            .iter()
            .map(|a| a.command.as_str())
            .collect();
        assert_eq!(
            commands,
            ["fmine assure public --synthetic", "fmine robot-docs"]
        );
    }

    #[test]
    fn test_cli_doctor_and_robot_docs() {
        let root = workspace_root();
        let env_doc = execute_command(&CliCommand::Doctor, &root);
        assert_eq!(env_doc.exit_code, 0);
        assert!(env_doc.to_json().contains("\"registries_ok\":true"));
        let n = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .sources
            .len();
        assert!(
            env_doc.to_json().contains(&format!(
                "{{\"issuer\":\"barrick\",\"registry_ok\":true,\"sources_registered\":{n},"
            )),
            "{}",
            env_doc.to_json()
        );

        let env_rob = execute_command(&CliCommand::RobotDocs, &root);
        assert_eq!(env_rob.exit_code, 0);
        let rob = env_rob.to_json();
        assert!(rob.contains("\"FMT-ENV-1\""));
        assert!(rob.contains("\"issuers\":[\"barrick\"]"), "{rob}");
        assert!(rob.contains("{\"code\":\"FMINE-REF-502\",\"exit_code\":8,"));
        // robot-docs documents every envelope field, in print order.
        let doc = &env_rob.payload_json[env_rob
            .payload_json
            .find("\"envelope_fields\":")
            .expect("envelope fields documented")..];
        let mut at = 0;
        for field in [
            "schema_version",
            "verb",
            "status",
            "exit_code",
            "anchor_id",
            "flags",
            "certificate",
            "warnings",
            "next_actions",
            "no_claim",
            "payload",
            "refusal",
        ] {
            at += doc[at..]
                .find(&format!("\"{field}\":"))
                .unwrap_or_else(|| panic!("{field} documented after the previous field: {doc}"));
        }
    }

    /// The four INV-002 flags, all false, written by hand from BD-329.S3 (not taken from `FLAGS_JSON`).
    const FLAGS: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                         \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

    const FLAG_NAMES: [&str; 4] = [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ];

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("fmine-cli-{tag}-{}", std::process::id()))
    }

    /// A workspace holding a copy of the barrick registry and no corpus.
    fn no_corpus_workspace(tag: &str) -> PathBuf {
        let ws = scratch(tag);
        let reg = registry_path(&ws, "barrick");
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        std::fs::copy(registry_path(&workspace_root(), "barrick"), &reg).expect("copy registry");
        ws
    }

    /// A synthetic issuer workspace: `assure public --issuer synthetic` runs there on invented numbers.
    fn synthetic_workspace(tag: &str) -> PathBuf {
        let ws = scratch(tag);
        fmine_public::synthetic::write_issuer_workspace(&ws).expect("synthetic workspace");
        ws
    }

    fn envelope_for(a: &[&str]) -> Envelope {
        invoke(&args(a), &workspace_root()).0
    }

    /// One envelope for each case of the PO-ADIT-1 golden, named.
    fn every_verb(ok_ws: &Path, no_corpus_ws: &Path) -> Vec<(&'static str, Envelope)> {
        let ok = ok_ws.display().to_string();
        let none = no_corpus_ws.display().to_string();
        vec![
            (
                "assure public ok",
                envelope_for(&[
                    "assure",
                    "public",
                    "--issuer",
                    "synthetic",
                    "--workspace",
                    &ok,
                ]),
            ),
            (
                "assure public refused",
                envelope_for(&[
                    "assure",
                    "public",
                    "--issuer",
                    "barrick",
                    "--workspace",
                    &none,
                ]),
            ),
            (
                "assure public --synthetic",
                envelope_for(&["assure", "public", "--synthetic"]),
            ),
            ("doctor", envelope_for(&["doctor"])),
            ("robot-docs", envelope_for(&["robot-docs"])),
            ("claim show", envelope_for(&["claim", "show", "clm_0001"])),
            ("usage error (parse_args)", envelope_for(&["frobnicate"])),
            (
                "usage error (execute_command)",
                execute_command(
                    &CliCommand::AssurePublic {
                        issuer: "newmont".to_string(),
                        through: None,
                    },
                    &workspace_root(),
                ),
            ),
        ]
    }

    /// `env` as JSON with the values that move with the ledger masked (an ok payload, the anchor id and the
    /// certificate) and the refusal's `what` text, which is refusal content rather than envelope frame (one names
    /// a scratch path; the claim show text is checked in test_cli_claim_show_refuses_without_a_claim_store). The
    /// refusal code, exit code and fix stay, so BD-328's code migration (bd-h0q1.3) shows up here.
    fn frame(env: &Envelope) -> String {
        let mut e = env.clone();
        if e.status == "ok" {
            e.payload_json = "\"<payload>\"".to_string();
        }
        e.anchor_id = e.anchor_id.map(|_| "<anchor>".to_string());
        e.certificate = e.certificate.map(|_| "<certificate>".to_string());
        if let Some(r) = e.refusal.as_mut() {
            r.what = "<what>".to_string();
        }
        e.to_json()
    }

    /// PO-ADIT-1: every verb's envelope, frame by frame. The expected frames are written by hand from the
    /// BD-329.S3 spec, not captured from this code's output.
    #[test]
    fn test_po_adit_1_golden_every_verb() {
        let ok_ws = synthetic_workspace("golden-ok");
        let none_ws = no_corpus_workspace("golden-none");
        let head = |verb: &str, status: &str, exit: u8, anchor: &str| {
            format!(
                "{{\"schema_version\":\"fmine.env.v1\",\"verb\":\"{verb}\",\"status\":\"{status}\",\
                 \"exit_code\":{exit},\"anchor_id\":{anchor},\"flags\":{FLAGS},"
            )
        };
        let action =
            |command: &str, why: &str| format!("{{\"command\":\"{command}\",\"why\":\"{why}\"}}");
        let synthetic_run = action(
            "fmine assure public --synthetic",
            "known-answer run on synthetic workbooks that exercises every ledger state",
        );
        let robot_docs = action("fmine robot-docs", "verbs, envelope fields and exit codes");
        let doctor = action(
            "fmine doctor",
            "which registered files are present, and whether every registry loads",
        );
        let no_claim = "\"no_claim\":\"Reproduces and checks the issuer's published AISC reconciliation tables \
                        as printed. It makes no reserve, resource, geotechnical, regulatory or investment claim \
                        and implies no issuer affiliation.\",";
        // The issuer name `synthetic` is the synthetic lane, so both ok runs here point to the barrick run;
        // the public-lane actions are checked on the real corpus (test_cli_assure_public_barrick_real_files).
        let barrick_run = action(
            "fmine assure public --issuer barrick",
            "the gate-eligible run over the workbooks in registries/sources/barrick.toml",
        );
        let fix_502 =
            "Re-verify input file integrity or regenerate the bundle from canonical sources.";
        let expected = [
            format!(
                "{}\"certificate\":\"<certificate>\",\"warnings\":[],\"next_actions\":[{barrick_run},\
                 {robot_docs}],{no_claim}\"payload\":\"<payload>\",\"refusal\":null}}",
                head("assure public", "ok", 0, "\"<anchor>\"")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-502\",\"exit_code\":8,\"what\":\"<what>\",\"fix\":\"{fix_502}\"}}}}",
                head("assure public", "refused", 8, "null")
            ),
            format!(
                "{}\"certificate\":\"<certificate>\",\"warnings\":[],\"next_actions\":[{barrick_run},\
                 {robot_docs}],{no_claim}\"payload\":\"<payload>\",\"refusal\":null}}",
                head("assure public", "ok", 0, "\"<anchor>\"")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{synthetic_run}],\
                 \"payload\":\"<payload>\",\"refusal\":null}}",
                head("doctor", "ok", 0, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\
                 \"payload\":\"<payload>\",\"refusal\":null}}",
                head("robot-docs", "ok", 0, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-502\",\"exit_code\":8,\"what\":\"<what>\",\
                 \"fix\":\"{fix_502}\"}}}}",
                head("claim show", "refused", 8, "null"),
                action(
                    "fmine assure public --issuer barrick",
                    "claims exist only inside an assure public report"
                )
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{robot_docs}],\
                 \"payload\":{{\"error\":\"unknown verb 'frobnicate'\"}},\"refusal\":null}}",
                head("unknown", "usage_error", 2, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{robot_docs}],\
                 \"payload\":{{\"error\":\"unknown issuer 'newmont' (registered: barrick)\"}},\"refusal\":null}}",
                head("assure public", "usage_error", 2, "null")
            ),
        ];
        let cases = every_verb(&ok_ws, &none_ws);
        assert_eq!(cases.len(), expected.len());
        for ((case, env), want) in cases.iter().zip(&expected) {
            assert_eq!(&frame(env), want, "{case}");
            // The unmasked envelope: the four flags, all false, exactly once, and no flag named anywhere else.
            let json = env.to_json();
            assert_eq!(json.matches(FLAGS).count(), 1, "{case}: {json}");
            for name in FLAG_NAMES {
                assert_eq!(json.matches(name).count(), 1, "{case}: {name}");
            }
        }
        let _ = std::fs::remove_dir_all(&ok_ws);
        let _ = std::fs::remove_dir_all(&none_ws);
    }

    #[test]
    fn test_flags_constant_in_every_constructor() {
        for env in [
            Envelope::ok("doctor", "{}".to_string(), Vec::new()),
            Envelope::report(
                "assure public",
                "anc_0".to_string(),
                "00".to_string(),
                "{}".to_string(),
                Vec::new(),
            ),
            Envelope::usage_error("unknown", "bad"),
            Envelope::refused(
                "claim show",
                Refusal::from_code("FMINE-REF-502", "absent"),
                Vec::new(),
            ),
        ] {
            let json = env.to_json();
            assert_eq!(json.matches(FLAGS).count(), 1, "{json}");
            assert_eq!(json.matches("\"flags\":").count(), 1, "{json}");
        }
    }

    /// Lines of `text` that write one of the four flags true, or name a setter for one.
    fn true_flag_writes(text: &str) -> Vec<String> {
        text.lines()
            .filter(|line| {
                FLAG_NAMES.iter().any(|f| {
                    [
                        format!("{f}\\\":true"),
                        format!("{f}\":true"),
                        format!("{f}\": true"),
                        format!("{f}: true"),
                        format!("{f} = true"),
                        format!("set_{f}"),
                    ]
                    .iter()
                    .any(|p| line.contains(p.as_str()))
                })
            })
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn test_no_source_writes_a_true_flag() {
        // The scan finds planted writes (built at run time, so this file never holds one) before it is
        // trusted with the tree.
        for f in FLAG_NAMES {
            for planted in [
                format!("let s = \"{{\\\"{f}\\\":true}}\";"),
                format!("let j = r#\"{{\"{f}\": true}}\"#;"),
                format!("report.set_{f}(true);"),
                format!("Flags {{ {f}: true }}"),
                format!("self.{f} = true;"),
            ] {
                assert_eq!(true_flag_writes(&planted).len(), 1, "{planted}");
            }
        }
        let mut files = Vec::new();
        let mut dirs = vec![workspace_root().join("crates")];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("read dir") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && path.components().any(|c| c.as_os_str() == "src")
                {
                    files.push(path);
                }
            }
        }
        assert!(files.len() > 16, "scanned only {} files", files.len());
        let hits: Vec<String> = files
            .iter()
            .flat_map(|p| {
                let text = std::fs::read_to_string(p).expect("read source");
                true_flag_writes(&text)
                    .into_iter()
                    .map(move |line| format!("{}: {line}", p.display()))
            })
            .collect();
        assert!(hits.is_empty(), "{hits:#?}");
    }

    /// Whether `command` parses as an `fmine` invocation, or names a repository path that exists.
    fn resolves(command: &str, root: &Path) -> bool {
        match command.strip_prefix("fmine ") {
            Some(rest) => {
                let argv: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
                parse_args(&argv, root).is_ok()
            }
            None => root.join(command).exists(),
        }
    }

    #[test]
    fn test_next_actions_resolvable() {
        let root = workspace_root();
        assert!(!resolves("fmine frobnicate", &root));
        assert!(!resolves("fmine doctor --deep", &root));
        assert!(!resolves("fmine assure public --issuer newmont", &root));
        assert!(!resolves("repro/no-such-issuer/fetch.sh", &root));
        let ok_ws = synthetic_workspace("actions-ok");
        let none_ws = no_corpus_workspace("actions-none");
        let mut checked = 0;
        for (case, env) in every_verb(&ok_ws, &none_ws) {
            assert!(!env.next_actions.is_empty(), "{case}");
            for a in &env.next_actions {
                assert!(resolves(&a.command, &root), "{case}: {}", a.command);
                checked += 1;
            }
        }
        assert_eq!(checked, 10);
        let _ = std::fs::remove_dir_all(&ok_ws);
        let _ = std::fs::remove_dir_all(&none_ws);
    }

    #[test]
    fn test_usage_error_from_parse_args_is_envelope() {
        let root = workspace_root();
        for (bad, verb) in [
            (&["frobnicate"][..], "unknown"),
            (&[][..], "unknown"),
            (
                &["--format", "md", "assure", "private"][..],
                "assure public",
            ),
            (&["--format", "xml", "doctor"][..], "doctor"),
            (&["claim", "show"][..], "claim show"),
        ] {
            let (text, exit) = run(&args(bad), &root);
            assert_eq!(exit, 2, "{bad:?}");
            assert_eq!(text.lines().count(), 1, "{bad:?}: {text}");
            assert!(
                text.starts_with(&format!(
                    "{{\"schema_version\":\"fmine.env.v1\",\"verb\":\"{verb}\",\"status\":\"usage_error\",\
                     \"exit_code\":2,\"anchor_id\":null,\"flags\":{FLAGS},"
                )),
                "{bad:?}: {text}"
            );
            assert!(text.ends_with(",\"refusal\":null}"), "{bad:?}: {text}");
        }
    }

    #[test]
    fn test_refused_502_points_to_doctor() {
        let ws = no_corpus_workspace("refused-502");
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "barrick",
            "--workspace",
            &ws.display().to_string(),
        ]);
        assert_eq!(env.refusal.as_ref().map(|r| r.code), Some("FMINE-REF-502"));
        let commands: Vec<&str> = env
            .next_actions
            .iter()
            .map(|a| a.command.as_str())
            .collect();
        assert_eq!(commands, ["fmine doctor"]);
        assert_eq!((env.certificate.as_deref(), env.no_claim), (None, None));
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_certificate_is_state_root_for_reports() {
        let report = fmine_public::run_synthetic_assurance(&[]).expect("synthetic run");
        let env = envelope_for(&["assure", "public", "--synthetic"]);
        assert_eq!(
            env.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
        assert_eq!(
            env.anchor_id,
            Some(report.ledger_anchor.anchor_id().to_string())
        );
        assert_eq!(env.no_claim, Some(fmine_public::NO_CLAIM));

        let ws = synthetic_workspace("certificate");
        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry");
        let report = run_public_assurance(&profile, &ws).expect("public-lane run");
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "synthetic",
            "--workspace",
            &ws.display().to_string(),
        ]);
        assert_eq!(
            env.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
        assert_ne!(
            env.certificate, env.anchor_id,
            "the certificate is the root, not the anchor id"
        );

        for env in [
            envelope_for(&["doctor"]),
            envelope_for(&["robot-docs"]),
            envelope_for(&["claim", "show", "clm_0001"]),
            envelope_for(&["frobnicate"]),
        ] {
            assert_eq!(
                (env.certificate.as_deref(), env.no_claim),
                (None, None),
                "{}",
                env.verb
            );
            assert!(
                env.to_json().contains(",\"certificate\":null,"),
                "{}",
                env.verb
            );
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_assure_payload_has_no_envelope_fields() {
        let ws = synthetic_workspace("payload");
        for env in [
            envelope_for(&["assure", "public", "--synthetic"]),
            envelope_for(&[
                "assure",
                "public",
                "--issuer",
                "synthetic",
                "--workspace",
                &ws.display().to_string(),
            ]),
        ] {
            assert_eq!(env.status, "ok", "{}", env.to_json());
            for field in ["\"flags\"", "\"no_claim\"", "\"next_actions\""] {
                assert!(!env.payload_json.contains(field), "{field} in the payload");
                assert_eq!(
                    env.to_json().matches(field).count(),
                    1,
                    "{field} once in the envelope"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&ws);
    }
}
