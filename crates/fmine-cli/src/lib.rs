//! FrankenMining CLI (`fmine`) implementation (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

mod forecast;
mod guidance;
mod guidance_capacity;
pub mod issued;
mod lint;
pub mod packet;
pub mod reliance;
mod reliance_binding;
pub mod reliance_explicit;
mod track_record;
mod track_record_data;
mod track_record_price;

use std::path::{Path, PathBuf};

use fmine_core::hash::sha256;
use fmine_evidence::{CATALOG, Refusal, RefusalCode};
use fmine_metrics::cutoff;
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
    /// Local issuance status, recall and owner-authorized journal events (BD-347).
    RelianceExplicit(reliance_explicit::Command),
    /// Verified assurance issuance or gated current-use retrieval (BD-347).
    Issued(issued::Command),
    /// `fmine assure public --issuer <issuer> [--through YYYYQn]` (`BD-071`): the gate-eligible run over the
    /// filings in `registries/sources/<issuer>.toml`, optionally only those at or before a quarter.
    AssurePublic {
        issuer: String,
        through: Option<String>,
    },
    /// `fmine assure public --synthetic`: the known-answer run over synthetic workbooks (never gate-eligible).
    AssureSynthetic,
    /// Prepare cited internal-review evidence challenges from an assurance snapshot (BD-349).
    AssureChallenges {
        /// `None` selects the explicitly requested synthetic lane.
        issuer: Option<String>,
        through: Option<String>,
    },
    /// Necessary conditions for completing cited annual guidance under historical sales persistence (BD-342).
    GuidanceCompletion {
        issuer: String,
        site: String,
        through: String,
        as_of: String,
        /// Optional registered technical report with three explicit physical upper limits.
        capacity_document: Option<String>,
    },
    /// Invented evidence exercising the same guidance completion adapter and exact kernel.
    GuidanceSynthetic,
    /// Retrospective guidance report, optionally limited to a workbook release.
    TrackRecord {
        /// `None` selects the explicitly requested synthetic lane.
        issuer: Option<String>,
        site: Option<String>,
        /// Selects source releases, not a historical information-availability cut.
        through: Option<String>,
        /// Explicit commodity-price subject; a different subject is a disclosed proxy, never inferred.
        price_subject: Option<String>,
    },
    /// Compare earliest registered guidance and paired outcomes at an explicit evidence cut.
    TrackRecordAsOf {
        issuer: String,
        site: Option<String>,
        as_of: String,
    },
    /// Descriptive recovery comparison against an agreed historical grade/tails relation.
    RecoveryGap {
        issuer: String,
        /// Selects workbook releases, not a historical information or forecast cut.
        through: Option<String>,
    },
    /// Invented annual and quarterly observations through the same recovery models.
    RecoverySynthetic,
    /// Check cited draft equalities against a verified public Claim snapshot (BD-345).
    Lint {
        draft: PathBuf,
        /// `None` selects the explicitly requested synthetic lane.
        issuer: Option<String>,
        through: Option<String>,
        /// Search the selected snapshot for uncited, fully specified equalities (BD-345.S3).
        discover: bool,
    },
    /// Replay a forecast transcript and assess its frozen cohort without dropping missing outcomes (BD-343).
    ForecastAssess {
        ledger: PathBuf,
        cohort: String,
        as_of: String,
        selector: String,
        /// Optional complete ancestry, checked at each forecast's own registration cut.
        provenance: Option<PathBuf>,
    },
    /// Issue, withdraw, supersede, inspect and gate reuse of exact report artifacts (BD-347).
    Reliance(reliance::Command),
    /// `fmine doctor` (`BD-115`).
    Doctor,
    /// Read an existing public Claim from a verified issuer snapshot or the explicit synthetic lane
    /// (`BD-115`). Without a selector, search the current canonical projections.
    ClaimShow {
        id: String,
        issuer: Option<String>,
        through: Option<String>,
        synthetic: bool,
    },
    /// `fmine claim why <id>`: evidence, interpretation and the actual Claim admission guards.
    ClaimWhy {
        id: String,
        issuer: Option<String>,
        through: Option<String>,
        synthetic: bool,
    },
    /// `fmine claim trace <id>`: source anchors and the available provenance boundary.
    ClaimTrace {
        id: String,
        issuer: Option<String>,
        through: Option<String>,
        synthetic: bool,
    },
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
\"payload\":\"the verb's result, refusal diagnostics when available, otherwise null\",\"refusal\":\"{code, exit_code, what, fix}, or null\"}";

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
    /// Optional human rendition of the same result. JSON envelopes never include presentation text.
    pub rendered_markdown: Option<String>,
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
            rendered_markdown: None,
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
        OutputFormat::Markdown => env.rendered_markdown.clone().unwrap_or_else(|| {
            format!(
                "# fmine `{}` ({})\n\n```json\n{}\n```",
                env.verb,
                env.status,
                env.to_json()
            )
        }),
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
            "--format"
            | "--workspace"
            | "--issuer"
            | "--through"
            | "--site"
            | "--as-of"
            | "--price-subject"
            | "--ledger"
            | "--cohort"
            | "--selector"
            | "--provenance"
            | "--capacity-document" => i += 2,
            flag if flag.starts_with("--") => i += 1,
            "assure" => {
                return match args.get(i + 1).map(String::as_str) {
                    Some("challenges") => "assure challenges",
                    Some("issue") => "assure issue",
                    _ => "assure public",
                };
            }
            "recall" => return "recall",
            "reliance" => {
                return match args.get(i + 1).map(String::as_str) {
                    Some("init") => "reliance init",
                    Some("withdraw") => "reliance withdraw",
                    Some("supersede") => "reliance supersede",
                    Some("refresh") => "reliance refresh",
                    Some("export") => "reliance export",
                    Some("issue") => "reliance issue",
                    Some("reuse") => "reliance reuse",
                    Some("recall") => "reliance recall",
                    _ => "reliance check",
                };
            }
            "guidance" => return "guidance completion",
            "track-record" => return "track-record",
            "recovery" => return "recovery gap",
            "lint" => return "lint",
            "forecast" => return "forecast assess",
            "claim" => {
                i += 1;
                while let Some(arg) = args.get(i) {
                    match arg.as_str() {
                        "--format"
                        | "--workspace"
                        | "--issuer"
                        | "--through"
                        | "--site"
                        | "--as-of"
                        | "--price-subject"
                        | "--capacity-document" => i += 2,
                        flag if flag.starts_with("--") => i += 1,
                        "why" => return "claim why",
                        "trace" => return "claim trace",
                        _ => return "claim show",
                    }
                }
                return "claim show";
            }
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
    if let Some(command) = parse_reliance_args(args, default_root)? {
        return Ok(command);
    }
    let mut format = OutputFormat::Json;
    let mut workspace = default_root.to_path_buf();
    let mut positional = Vec::new();
    let mut issuer: Option<String> = None;
    let mut synthetic = false;
    let mut through = None;
    let mut site = None;
    let mut price_subject = None;
    let mut as_of = None;
    let mut ledger = None;
    let mut cohort = None;
    let mut selector = None;
    let mut capacity_document = None;
    let mut discover = false;
    let mut provenance = None;

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
            "--json" => format = OutputFormat::Json,
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
            "--discover" => discover = true,
            "--site" => {
                i += 1;
                site = Some(args.get(i).ok_or("missing value for --site")?.clone());
            }
            "--price-subject" => {
                i += 1;
                let subject = args.get(i).ok_or("missing value for --price-subject")?;
                if subject.trim().is_empty() || subject.chars().any(char::is_control) {
                    return Err(
                        "--price-subject requires a nonempty subject without control characters"
                            .into(),
                    );
                }
                price_subject = Some(subject.clone());
            }
            "--as-of" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --as-of")?;
                if !fmine_registry::sources::is_date(value)
                    && fmine_calib::record::parse_utc(value).is_none()
                {
                    return Err("--as-of must be YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ".to_string());
                }
                as_of = Some(value.clone());
            }
            "--ledger" => {
                i += 1;
                ledger = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --ledger")?,
                ));
            }
            "--cohort" => {
                i += 1;
                cohort = Some(args.get(i).ok_or("missing value for --cohort")?.clone());
            }
            "--selector" => {
                i += 1;
                selector = Some(args.get(i).ok_or("missing value for --selector")?.clone());
            }
            "--capacity-document" => {
                i += 1;
                let document = args.get(i).ok_or("missing value for --capacity-document")?;
                if document.trim().is_empty() {
                    return Err("--capacity-document requires a registered filename".to_string());
                }
                capacity_document = Some(document.clone());
            }
            "--provenance" => {
                i += 1;
                provenance = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --provenance")?,
                ));
            }
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
        "assure" | "guidance" | "recovery" | "forecast" | "lint" => 2,
        "claim" => 3,
        "doctor" | "robot-docs" | "track-record" => 1,
        other => return Err(format!("unknown verb '{other}'")),
    };
    if let Some(extra) = positional.get(words) {
        return Err(format!(
            "unexpected argument '{extra}' after '{}'",
            positional[..words].join(" ")
        ));
    }
    if !matches!(
        positional[0].as_str(),
        "assure" | "claim" | "guidance" | "recovery" | "lint" | "track-record"
    ) && (issuer.is_some() || synthetic || through.is_some())
    {
        return Err(
            "--issuer, --synthetic and --through require a source-reading command".to_string(),
        );
    }
    if !matches!(positional[0].as_str(), "guidance" | "track-record") && site.is_some() {
        return Err("--site requires 'guidance completion' or 'track-record'".to_string());
    }
    if positional[0] != "track-record" && price_subject.is_some() {
        return Err("--price-subject requires 'track-record'".to_string());
    }
    if !matches!(
        positional[0].as_str(),
        "guidance" | "forecast" | "track-record"
    ) && as_of.is_some()
    {
        return Err(
            "--as-of requires 'guidance completion', 'track-record' or 'forecast assess'"
                .to_string(),
        );
    }
    if positional[0] != "guidance" && capacity_document.is_some() {
        return Err("--capacity-document requires 'guidance completion'".to_string());
    }
    if positional[0] != "forecast"
        && (ledger.is_some() || cohort.is_some() || selector.is_some() || provenance.is_some())
    {
        return Err(
            "--ledger, --cohort, --selector and --provenance require 'forecast assess'".to_string(),
        );
    }
    if discover && positional[0] != "lint" {
        return Err("--discover requires 'lint'".to_string());
    }

    let cmd = match positional[0].as_str() {
        "assure" => {
            let challenges = positional.get(1).map(String::as_str) == Some("challenges");
            if !challenges && positional.get(1).map(String::as_str) != Some("public") {
                return Err("expected 'fmine assure public --issuer <issuer>' or 'fmine assure public --synthetic'".to_string());
            }
            let registered = || registered_issuers(&workspace).join(", ");
            let selected = match (issuer.as_deref(), synthetic) {
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
            };
            if challenges {
                match selected {
                    CliCommand::AssurePublic { issuer, through } => CliCommand::AssureChallenges {
                        issuer: Some(issuer),
                        through,
                    },
                    CliCommand::AssureSynthetic => CliCommand::AssureChallenges {
                        issuer: None,
                        through: None,
                    },
                    _ => return Err("expected an assurance source".to_string()),
                }
            } else {
                selected
            }
        }
        "guidance" => {
            if positional.get(1).map(String::as_str) != Some("completion") {
                return Err("expected 'fmine guidance completion'".to_string());
            }
            if synthetic {
                if issuer.is_some()
                    || site.is_some()
                    || through.is_some()
                    || as_of.is_some()
                    || capacity_document.is_some()
                {
                    return Err(
                        "guidance --synthetic takes no issuer, site, through, as-of or capacity-document selection"
                            .to_string(),
                    );
                }
                CliCommand::GuidanceSynthetic
            } else {
                if as_of
                    .as_deref()
                    .is_some_and(|v| !fmine_registry::sources::is_date(v))
                {
                    return Err("guidance completion --as-of requires YYYY-MM-DD".to_string());
                }
                let issuer =
                    issuer.ok_or("guidance completion requires --issuer or --synthetic")?;
                if !is_issuer_name(&issuer) || !registry_path(&workspace, &issuer).is_file() {
                    return Err(format!("unknown issuer '{issuer}'"));
                }
                CliCommand::GuidanceCompletion {
                    issuer,
                    site: site
                        .filter(|s| !s.is_empty())
                        .ok_or("guidance completion requires --site")?,
                    through: through.ok_or("guidance completion requires --through YYYYQn")?,
                    as_of: as_of.ok_or("guidance completion requires --as-of YYYY-MM-DD")?,
                    capacity_document,
                }
            }
        }
        "track-record" => {
            if through.is_some() && as_of.is_some() {
                return Err("track-record selects either --through or --as-of".to_string());
            }
            if as_of.is_some() && price_subject.is_some() {
                return Err("track-record --as-of cannot be combined with --price-subject; price sensitivity is available in retrospective mode".to_string());
            }
            if synthetic {
                if issuer.is_some()
                    || site.is_some()
                    || through.is_some()
                    || as_of.is_some()
                    || price_subject.is_some()
                {
                    return Err(
                        "track-record --synthetic takes no issuer, site, through, as-of or price-subject selection"
                            .to_string(),
                    );
                }
                CliCommand::TrackRecord {
                    issuer: None,
                    site: None,
                    through: None,
                    price_subject: None,
                }
            } else {
                let issuer = issuer.ok_or("track-record requires --issuer or --synthetic")?;
                if !is_issuer_name(&issuer) || !registry_path(&workspace, &issuer).is_file() {
                    return Err(format!("unknown issuer '{issuer}'"));
                }
                if site.as_deref().is_some_and(|value| value.trim().is_empty()) {
                    return Err("track-record --site requires a nonempty subject key".to_string());
                }
                if let Some(as_of) = as_of {
                    if !fmine_registry::sources::is_date(&as_of) {
                        return Err("track-record --as-of requires YYYY-MM-DD".to_string());
                    }
                    CliCommand::TrackRecordAsOf {
                        issuer,
                        site,
                        as_of,
                    }
                } else {
                    CliCommand::TrackRecord {
                        issuer: Some(issuer),
                        site,
                        through,
                        price_subject,
                    }
                }
            }
        }
        "recovery" => {
            if positional.get(1).map(String::as_str) != Some("gap") {
                return Err("expected 'fmine recovery gap'".to_string());
            }
            if synthetic {
                if issuer.is_some() || through.is_some() {
                    return Err(
                        "recovery --synthetic takes no issuer or through selection".to_string()
                    );
                }
                CliCommand::RecoverySynthetic
            } else {
                let issuer = issuer.ok_or("recovery gap requires --issuer or --synthetic")?;
                if !is_issuer_name(&issuer) || !registry_path(&workspace, &issuer).is_file() {
                    return Err(format!("unknown issuer '{issuer}'"));
                }
                CliCommand::RecoveryGap { issuer, through }
            }
        }
        "lint" => {
            let draft = positional
                .get(1)
                .ok_or("lint requires a local draft path")?;
            if issuer.is_some() && synthetic || issuer.is_none() && !synthetic {
                return Err("lint requires --issuer <issuer> or --synthetic".to_string());
            }
            if through.is_some() && issuer.is_none() {
                return Err("--through applies to --issuer runs only".to_string());
            }
            if let Some(name) = &issuer
                && (!is_issuer_name(name) || !registry_path(&workspace, name).is_file())
            {
                return Err(format!("unknown issuer '{name}'"));
            }
            CliCommand::Lint {
                draft: PathBuf::from(draft),
                issuer,
                through,
                discover,
            }
        }
        "forecast" => {
            if positional.get(1).map(String::as_str) != Some("assess") {
                return Err("expected 'fmine forecast assess'".to_string());
            }
            let as_of = as_of.ok_or("forecast assess requires --as-of YYYY-MM-DDTHH:MM:SSZ")?;
            if fmine_calib::record::parse_utc(&as_of).is_none() {
                return Err("forecast assess --as-of requires YYYY-MM-DDTHH:MM:SSZ".to_string());
            }
            let selector = selector.unwrap_or_else(|| "first-published".to_string());
            if !matches!(selector.as_str(), "first-published" | "latest-as-of") {
                return Err("--selector must be first-published or latest-as-of".to_string());
            }
            CliCommand::ForecastAssess {
                ledger: ledger.ok_or("forecast assess requires --ledger <path>")?,
                cohort: cohort
                    .filter(|v| !v.trim().is_empty())
                    .ok_or("forecast assess requires --cohort <name>")?,
                as_of,
                selector,
                provenance,
            }
        }
        "doctor" => CliCommand::Doctor,
        "claim" => {
            let action = positional.get(1).map(String::as_str);
            let Some(action @ ("show" | "why" | "trace")) = action else {
                return Err("expected 'fmine claim show|why|trace <id>'".to_string());
            };
            let id = positional
                .get(2)
                .ok_or_else(|| format!("missing <id> for 'fmine claim {action} <id>'"))?
                .clone();
            if issuer.is_some() && synthetic {
                return Err("--issuer and --synthetic are exclusive".to_string());
            }
            if through.is_some() && issuer.is_none() {
                return Err("--through applies to --issuer runs only".to_string());
            }
            if let Some(name) = &issuer
                && (!is_issuer_name(name) || !registry_path(&workspace, name).is_file())
            {
                return Err(format!("unknown issuer '{name}'"));
            }
            match action {
                "why" => CliCommand::ClaimWhy {
                    id,
                    issuer,
                    through,
                    synthetic,
                },
                "trace" => CliCommand::ClaimTrace {
                    id,
                    issuer,
                    through,
                    synthetic,
                },
                _ => CliCommand::ClaimShow {
                    id,
                    issuer,
                    through,
                    synthetic,
                },
            }
        }
        "robot-docs" => CliCommand::RobotDocs,
        other => return Err(format!("unknown verb '{other}'")),
    };

    Ok((cmd, format, workspace))
}

/// DESIGN (BD-347): authority commands have their own strict argument grammar. Only presentation and
/// workspace selection are global; owner keys, pins and timestamps are never inferred by this router.
fn parse_reliance_args(
    args: &[String],
    default_root: &Path,
) -> Result<Option<(CliCommand, OutputFormat, PathBuf)>, String> {
    let mut command = Vec::new();
    let mut flags = Vec::new();
    let mut format = OutputFormat::Json;
    let mut workspace = default_root.to_path_buf();
    let mut i = 0;
    while let Some(arg) = args.get(i) {
        match arg.as_str() {
            "--json" => format = OutputFormat::Json,
            "--format" => {
                i += 1;
                format = match args.get(i).map(String::as_str) {
                    Some("json") => OutputFormat::Json,
                    Some("markdown" | "md") => OutputFormat::Markdown,
                    Some(other) => {
                        return Err(format!(
                            "unsupported --format '{other}' (expected json|markdown)"
                        ));
                    }
                    None => return Err("missing value for --format".to_string()),
                };
            }
            "--workspace" => {
                i += 1;
                workspace = PathBuf::from(
                    args.get(i)
                        .filter(|value| !value.is_empty() && !value.starts_with("--"))
                        .ok_or("missing value for --workspace")?,
                );
            }
            flag if flag.starts_with("--") => {
                flags.push(flag);
                command.push(arg.clone());
                if !matches!(
                    flag,
                    "--synthetic" | "--trust-host-clock" | "--all-claims" | "--require-current"
                ) && let Some(value) = args.get(i + 1)
                {
                    // DESIGN: preserve even malformed flag-looking values for the strict parser.
                    // Stripping a global option here would silently repair a missing authority value.
                    command.push(value.clone());
                    i += 1;
                }
            }
            _ => command.push(arg.clone()),
        }
        i += 1;
    }
    let first = command.first().map(String::as_str);
    let second = command.get(1).map(String::as_str);
    let explicit = flags.iter().any(|flag| {
        matches!(
            *flag,
            "--key"
                | "--expected-head"
                | "--now"
                | "--uncertainty-ns"
                | "--max-age-ns"
                | "--max-skew-ns"
        )
    });
    let parsed = match (first, second) {
        (Some("assure"), Some("issue")) | (Some("reliance"), Some("export")) => {
            CliCommand::Issued(issued::parse(&command, &workspace)?)
        }
        (Some("reliance"), Some("issue" | "reuse" | "recall")) => {
            return reliance::parse(args, default_root).map(|(command, format, root)| {
                Some((CliCommand::Reliance(command), format, root))
            });
        }
        (Some("reliance"), Some("check")) if flags.contains(&"--digest") => {
            return reliance::parse(args, default_root).map(|(command, format, root)| {
                Some((CliCommand::Reliance(command), format, root))
            });
        }
        (Some("reliance"), Some("check")) | (Some("recall"), _) => {
            CliCommand::RelianceExplicit(reliance_explicit::parse(&command)?)
        }
        (Some("reliance"), _) if explicit => {
            CliCommand::RelianceExplicit(reliance_explicit::parse(&command)?)
        }
        (Some("reliance"), _) => {
            return reliance::parse(args, default_root).map(|(command, format, root)| {
                Some((CliCommand::Reliance(command), format, root))
            });
        }
        _ => return Ok(None),
    };
    Ok(Some((parsed, format, workspace)))
}

/// The envelope of a finished `assure public` report.
/// The published cut-offs of an issuer run (BD-274, plan §15): `fmine_metrics::cutoff::section` over the workspace
/// registry, or `None` where the workspace registers no cut-offs for this issuer. A registry that does not read, or a
/// row that cannot be computed, refuses the run (`FMINE-REF-506`), so a report never shows fewer than are registered.
fn published_cutoffs(workspace_root: &Path, issuer: &str) -> Result<Option<String>, Refusal> {
    let refuse = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("{}: {why}", cutoff::CUTOFFS_REGISTRY),
        )
    };
    let bytes = match std::fs::read(workspace_root.join(cutoff::CUTOFFS_REGISTRY)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(refuse(e.to_string())),
    };
    match cutoff::section(&bytes) {
        Ok(section) => Ok((section.issuer == issuer).then_some(section.json)),
        Err(e) => Err(refuse(e.to_string())),
    }
}

/// The report's envelope. The cut-off section, when there is one, is the payload's last field; it is not in the
/// state root, so the certificate is the same with or without it.
fn report_envelope(
    report: &fmine_public::PublicAssuranceReport,
    cutoffs: Option<&str>,
) -> Envelope {
    let mut payload = report.to_json();
    if let Some(section) = cutoffs
        && payload.ends_with('}')
    {
        payload.pop();
        payload.push_str(",\"published_cutoffs\":");
        payload.push_str(section);
        payload.push('}');
    }
    Envelope::report(
        "assure public",
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        payload,
        report
            .next_actions()
            .into_iter()
            .map(|(command, why)| NextAction::new(command, why))
            .collect(),
    )
}

/// A Claim lookup is a read of the current verified projection, never construction from a supplied id.
/// Both the identity-ledger root and the keyed projection digest are exposed: interpretation changes can
/// change the latter without moving the former (BD-332). Inspection views retain this same snapshot.
fn public_claim_envelope(
    report: &fmine_public::PublicAssuranceReport,
    profile: &fmine_public::sources::IssuerProfile,
    id: &str,
    view: ClaimView,
    selector: &str,
) -> Envelope {
    let Some(cell) = report
        .public_claims
        .cells
        .values()
        .find(|cell| cell.claim.claim_id.to_string() == id)
    else {
        return Envelope::refused(
            view.verb(),
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "claim '{id}' is absent from the selected {} snapshot",
                    report.issuer
                ),
            ),
            vec![NextAction::new(
                format!("fmine assure public {selector}"),
                "the verified source snapshot containing these public Claims",
            )],
        );
    };
    let digest = fmine_public::key::claims_digest(report.public_claims.cells.values()).to_hex();
    let inspection = match view {
        ClaimView::Show => String::new(),
        ClaimView::Why => format!(",\"why\":{}", fmine_public::key::inspect::why_json(cell)),
        ClaimView::Trace => format!(
            ",\"trace\":{}",
            fmine_public::key::inspect::trace_json(cell, profile)
        ),
    };
    let payload = format!(
        "{{\"schema\":\"fmine.claim_read.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\
         \"read_mode\":\"verified_source_projection\",\"claims_digest\":\"{digest}\",\
         \"claim\":{},\"stale\":{},\"calibration_failed\":{},\"decision_uu_present\":{}{inspection}}}",
        escape_json(&report.issuer),
        report.lane.as_str(),
        cell.to_json(),
        cell.claim.stale,
        cell.claim.calibration_failed,
        !cell.claim.uu_terms.is_empty(),
    );
    Envelope::report(
        view.verb(),
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        payload,
        claim_actions(id, view, selector),
    )
}

fn challenge_envelope(report: &fmine_public::PublicAssuranceReport) -> Result<Envelope, Refusal> {
    let packets = packet::packets_from_report(report)?;
    let bodies: Vec<String> = packets
        .iter()
        .map(packet::ChallengePacket::to_json)
        .collect();
    let packet_set = format!("[{}]", bodies.join(","));
    let digest = sha256(packet_set.as_bytes()).to_hex();
    let payload = format!(
        "{{\"schema\":\"fmine.challenge_packets.v1\",\"issuer\":\"{}\",\"lane\":\"{}\",\
         \"source_state_root\":\"{}\",\"packet_set_sha256\":\"{digest}\",\"count\":{},\"packets\":{packet_set}}}",
        escape_json(&report.issuer),
        report.lane.as_str(),
        report.ledger_anchor.state_root.to_hex(),
        packets.len()
    );
    let mut env = Envelope::report(
        "assure challenges",
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        payload,
        report
            .next_actions()
            .into_iter()
            .map(|(command, why)| NextAction::new(command, why))
            .collect(),
    );
    env.rendered_markdown = Some(format!(
        "# Evidence challenge packets\n\n{}\n\nSource state root: `{}`\n\nPacket set SHA-256: `{digest}`\n\n{}",
        packet::REVIEW_BOUNDARY,
        report.ledger_anchor.state_root.to_hex(),
        packets
            .iter()
            .map(packet::ChallengePacket::to_markdown)
            .collect::<Vec<_>>()
            .join("\n---\n\n")
    ));
    Ok(env)
}

fn guidance_envelope(
    result: Result<fmine_public::guidance::GuidanceCompletionReport, Refusal>,
    selection: &str,
) -> Envelope {
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match result {
        Ok(report) => {
            let mut env = Envelope::ok("guidance completion", report.to_json(), actions);
            // The payload digest binds evidence and assumptions. This computation has no authority-ledger
            // registration, so both the ledger certificate and anchor deliberately remain absent.
            env.no_claim = Some(fmine_public::guidance::NO_CLAIM);
            env.rendered_markdown = Some(guidance::render(&report, selection));
            env
        }
        Err(refusal) => Envelope::refused("guidance completion", refusal, actions),
    }
}

fn guidance_capacity_envelope(
    report: &fmine_public::guidance::GuidanceCompletionReport,
    capacity: &guidance_capacity::GuidanceCapacityReport,
    selection: &str,
) -> Envelope {
    let body = format!(
        "\"schema\":\"fmine.guidance.capacity-completion.v1\",\"guidance\":{},\"capacity\":{}",
        report.to_json(),
        capacity.to_json()
    );
    let digest = fmine_core::hash::blake3_hash(body.as_bytes()).to_hex();
    let payload = format!("{{\"digest\":\"{digest}\",{body}}}");
    let mut env = Envelope::ok(
        "guidance completion",
        payload,
        vec![NextAction::new("fmine doctor", DOCTOR_WHY)],
    );
    env.no_claim = Some(
        "Arithmetic requirements, finite historical comparisons and a production-only ceiling from explicitly stated limits. No forecast, probability of meeting guidance, joint production-cost feasibility or regulatory claim.",
    );
    let mut markdown = guidance::render_with_capacity(report, selection, &capacity.to_markdown());
    markdown.push_str(&format!("\nComposite report BLAKE3: `{digest}`.\n"));
    env.rendered_markdown = Some(markdown);
    env
}

fn recovery_envelope(result: Result<fmine_public::recovery::RecoveryReport, Refusal>) -> Envelope {
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match result {
        Ok(report) => {
            let mut env = Envelope::ok("recovery gap", report.to_json(), actions);
            // The model's own digest binds its inputs and results; it is not an authority-ledger root.
            env.no_claim = Some(fmine_metrics::recovery::NO_CLAIM);
            env
        }
        Err(refusal) => Envelope::refused("recovery gap", refusal, actions),
    }
}

fn track_record_envelope(
    result: Result<fmine_public::guidance::track_record::TrackRecordReport, Refusal>,
) -> Envelope {
    let actions = vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match result {
        Ok(report) => {
            let mut env = Envelope::ok("track-record", report.to_json(), actions);
            // The report binds its cited evidence with a payload digest. Historical source selection does
            // not create an authority-ledger registration or an external timestamp proof.
            env.no_claim = Some(fmine_public::guidance::track_record::NO_CLAIM);
            env.rendered_markdown = Some(report.to_markdown());
            env
        }
        Err(refusal) => Envelope::refused("track-record", refusal, actions),
    }
}

/// Executes a parsed `CliCommand` against `workspace_root` and returns an `FMT-ENV-1` `Envelope`.
#[must_use]
pub fn execute_command(cmd: &CliCommand, workspace_root: &Path) -> Envelope {
    let to_doctor = || vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    match cmd {
        CliCommand::RelianceExplicit(command) => {
            reliance_explicit::execute(command, workspace_root)
        }
        CliCommand::Issued(command) => issued::execute(command, workspace_root),
        CliCommand::Reliance(command) => reliance::execute(command, workspace_root),
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
            let cutoffs = match published_cutoffs(workspace_root, issuer) {
                Ok(section) => section,
                Err(refusal) => return Envelope::refused("assure public", refusal, to_doctor()),
            };
            // BD-339 adds its reproduction kit to the refusal's next actions once it exists.
            match run_public_assurance(&profile, workspace_root) {
                Ok(report) => report_envelope(&report, cutoffs.as_deref()),
                Err(refusal) => Envelope::refused("assure public", refusal, to_doctor()),
            }
        }
        CliCommand::AssureSynthetic => match run_synthetic_assurance(&[]) {
            Ok(report) => report_envelope(&report, None),
            Err(refusal) => Envelope::refused("assure public", refusal, to_doctor()),
        },
        CliCommand::AssureChallenges { issuer, through } => {
            let report = match issuer {
                Some(name) => load_issuer_profile(workspace_root, name)
                    .and_then(|p| match through {
                        Some(bound) => p.through(bound),
                        None => Ok(p),
                    })
                    .and_then(|p| run_public_assurance(&p, workspace_root)),
                None if through.is_none() => run_synthetic_assurance(&[]),
                None => {
                    return Envelope::usage_error(
                        "assure challenges",
                        "--through requires --issuer",
                    );
                }
            };
            match report.and_then(|r| challenge_envelope(&r)) {
                Ok(env) => env,
                Err(refusal) => Envelope::refused("assure challenges", refusal, to_doctor()),
            }
        }
        CliCommand::GuidanceCompletion {
            issuer,
            site,
            through,
            as_of,
            capacity_document,
        } => {
            let result =
                fmine_public::guidance::run_public(workspace_root, issuer, site, through, as_of);
            let selection =
                format!("Issuer: {issuer}; actuals through {through}; evidence as of {as_of}.");
            match capacity_document {
                None => guidance_envelope(result, &selection),
                Some(document) => match result.and_then(|report| {
                    guidance_capacity::assess(
                        workspace_root,
                        issuer,
                        site,
                        through,
                        as_of,
                        document,
                        &report,
                    )
                    .map(|capacity| (report, capacity))
                }) {
                    Ok((report, capacity)) => {
                        guidance_capacity_envelope(&report, &capacity, &selection)
                    }
                    Err(refusal) => Envelope::refused("guidance completion", refusal, to_doctor()),
                },
            }
        }
        CliCommand::GuidanceSynthetic => guidance_envelope(
            fmine_public::guidance::run_synthetic(),
            "Invented synthetic evidence; this example is not an issuer finding.",
        ),
        CliCommand::TrackRecord {
            issuer,
            site,
            through,
            price_subject,
        } => track_record::execute(
            issuer.as_deref(),
            site.as_deref(),
            through.as_deref(),
            price_subject.as_deref(),
            workspace_root,
        ),
        CliCommand::TrackRecordAsOf {
            issuer,
            site,
            as_of,
        } => track_record_envelope(fmine_public::guidance::track_record::run(
            workspace_root,
            issuer,
            site.as_deref(),
            as_of,
        )),
        CliCommand::RecoveryGap { issuer, through } => recovery_envelope(
            fmine_public::recovery::run_public(workspace_root, issuer, through.as_deref()),
        ),
        CliCommand::RecoverySynthetic => recovery_envelope(fmine_public::recovery::run_synthetic()),
        CliCommand::Lint {
            draft,
            issuer,
            through,
            discover,
        } => lint::execute(
            draft,
            issuer.as_deref(),
            through.as_deref(),
            *discover,
            workspace_root,
        ),
        CliCommand::ForecastAssess {
            ledger,
            cohort,
            as_of,
            selector,
            provenance,
        } => {
            let path = workspace_root.join(ledger);
            let provenance = provenance.as_ref().map(|p| workspace_root.join(p));
            let actions = vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)];
            match forecast::assess(&path, cohort, as_of, selector, provenance.as_deref()) {
                Ok(payload) => {
                    let mut env = Envelope::ok("forecast assess", payload, actions);
                    env.no_claim = Some(
                        "Read-only assessment of supplied forecast records. Record times and source citations are not independently time-proved or source-verified; this does not issue a forecast or establish forecasting skill.",
                    );
                    env
                }
                Err(failure) => {
                    let mut env = Envelope::refused("forecast assess", failure.refusal, actions);
                    if let Some(diagnostics) = failure.diagnostics {
                        env.payload_json = diagnostics;
                    }
                    env
                }
            }
        }
        CliCommand::Doctor => match RegistryCatalog::load_from_root(workspace_root) {
            Ok(cat) => {
                // One entry per registries/sources/<issuer>.toml: registered, present and verified counts with
                // one row per file (BD-339.S2), or the registry's refusal (file and line) when it does not load.
                // A file whose bytes differ from the registry is reported, never refused: doctor diagnoses.
                let issuers: Vec<String> = registered_issuers(workspace_root)
                    .iter()
                    .map(|name| match load_issuer_profile(workspace_root, name) {
                        Ok(p) => {
                            let dir = workspace_root.join(&p.corpus_dir);
                            let checks: Vec<(&str, [bool; 3])> = p
                                .sources
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
                                    (s.file_name.as_str(), check)
                                })
                                .collect();
                            let count = |i: usize| checks.iter().filter(|(_, c)| c[i]).count();
                            let rows: Vec<String> = checks
                                .iter()
                                .map(|(file, [present, bytes_ok, digest_ok])| {
                                    format!(
                                        "{{\"file\":\"{}\",\"present\":{present},\"bytes_ok\":{bytes_ok},\"digest_ok\":{digest_ok}}}",
                                        escape_json(file)
                                    )
                                })
                                .collect();
                            format!(
                                "{{\"issuer\":\"{}\",\"registry_ok\":true,\"sources_registered\":{},\"sources_present\":{},\"sources_verified\":{},\"sources\":[{}]}}",
                                escape_json(name),
                                p.sources.len(),
                                count(0),
                                count(2),
                                rows.join(",")
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
            // The registries could not be read: foundation I/O, never a stale anchor or a digest code.
            Err(e) => Envelope::refused(
                "doctor",
                Refusal::new(RefusalCode::InputMissingOrUnreadable506, e.to_string()),
                vec![NextAction::new("fmine robot-docs", ROBOT_DOCS_WHY)],
            ),
        },
        CliCommand::ClaimShow {
            id,
            issuer,
            through,
            synthetic,
        }
        | CliCommand::ClaimWhy {
            id,
            issuer,
            through,
            synthetic,
        }
        | CliCommand::ClaimTrace {
            id,
            issuer,
            through,
            synthetic,
        } => {
            let view = match cmd {
                CliCommand::ClaimWhy { .. } => ClaimView::Why,
                CliCommand::ClaimTrace { .. } => ClaimView::Trace,
                _ => ClaimView::Show,
            };
            if issuer.is_some() && *synthetic || through.is_some() && issuer.is_none() {
                return Envelope::usage_error(
                    view.verb(),
                    "choose --issuer [--through YYYYQn] or --synthetic",
                );
            }
            let selected = if let Some(name) = issuer {
                if !is_issuer_name(name) || !registry_path(workspace_root, name).is_file() {
                    return Envelope::usage_error(view.verb(), &format!("unknown issuer '{name}'"));
                }
                let selector = format!(
                    "--issuer {name}{}",
                    through
                        .as_ref()
                        .map_or_else(String::new, |q| format!(" --through {q}"))
                );
                let loaded =
                    load_issuer_profile(workspace_root, name).and_then(|p| match through {
                        Some(bound) => p.through(bound),
                        None => Ok(p),
                    });
                Some((
                    selector,
                    loaded.and_then(|profile| {
                        run_public_assurance(&profile, workspace_root)
                            .map(|report| (report, profile))
                    }),
                ))
            } else if *synthetic {
                Some((
                    "--synthetic".to_string(),
                    run_synthetic_assurance(&[])
                        .map(|report| (report, fmine_public::synthetic::synthetic_profile())),
                ))
            } else {
                None
            };
            if let Some((selector, snapshot)) = selected {
                return match snapshot {
                    Ok((report, profile)) => {
                        public_claim_envelope(&report, &profile, id, view, &selector)
                    }
                    Err(refusal) => Envelope::refused(view.verb(), refusal, to_doctor()),
                };
            }
            inspect_public_claim(id, workspace_root, view)
        }
        CliCommand::RobotDocs => {
            let refusals: Vec<String> = CATALOG
                .iter()
                .map(|r| {
                    format!(
                        "{{\"code\":\"{}\",\"exit_code\":{},\"status\":\"{}\",\"summary\":\"{}\"}}",
                        r.code,
                        r.exit_code,
                        r.status.as_str(),
                        escape_json(r.summary)
                    )
                })
                .collect();
            let issuers: Vec<String> = registered_issuers(workspace_root)
                .iter()
                .map(|i| format!("\"{}\"", escape_json(i)))
                .collect();
            let verbs = [
                "assure public --issuer <issuer> [--through YYYYQn]",
                "assure public --synthetic",
                "assure challenges --issuer <issuer> [--through YYYYQn]",
                "assure challenges --synthetic",
                "assure issue (--issuer <issuer> [--through YYYYQn] | --synthetic) --output <path> --journal <path> --expected-head <hex> --key <path> --role <generic-role> --purpose <text> --now <UTC> --uncertainty-ns <n>",
                "reliance init --journal <path> --key <path> --now <UTC> --uncertainty-ns <n> --max-age-ns <n> --max-skew-ns <n>",
                "reliance check <digest|file> --journal <path> [--head <hex>] [--now <UTC> --uncertainty-ns <n>] [--as-of <UTC>]",
                "reliance withdraw <digest> --journal <path> --expected-head <hex> --key <path> --now <UTC> --uncertainty-ns <n> --reason <text> --defect code-defect|source-restated|scope-error --claim <id> [--claim <id>]",
                "reliance supersede <digest> --replacement <hex> --journal <path> --expected-head <hex> --key <path> --now <UTC> --uncertainty-ns <n>",
                "reliance refresh --journal <path> --expected-head <hex> --key <path> --now <UTC> --uncertainty-ns <n>",
                "reliance export <file> --output <path> --journal <path> --head <hex> --now <UTC> --uncertainty-ns <n>",
                "recall --journal <path> (--revision <build-revision> | --claim <id>) [--head <hex>]",
                "guidance completion --issuer <issuer> --site <site> --through YYYYQn --as-of YYYY-MM-DD [--capacity-document <registered file>]",
                "guidance completion --synthetic",
                "track-record --issuer <issuer> [--site <subject>] [--through YYYYQn] [--price-subject <subject>]",
                "track-record --issuer <issuer> [--site <site>] --as-of YYYY-MM-DD",
                "track-record --synthetic [--json]",
                "recovery gap --issuer <issuer> [--through YYYYQn]",
                "recovery gap --synthetic",
                "lint <draft> --issuer <issuer> [--through YYYYQn] [--discover]",
                "lint <draft> --synthetic [--discover]",
                "forecast assess --ledger <path> --cohort <name> --as-of YYYY-MM-DDTHH:MM:SSZ [--selector first-published|latest-as-of] [--provenance <path>]",
                "doctor",
                "claim show <id> [--issuer <issuer> [--through YYYYQn] | --synthetic]",
                "claim why <id> [--issuer <issuer> [--through YYYYQn] | --synthetic]",
                "claim trace <id> [--issuer <issuer> [--through YYYYQn] | --synthetic]",
                "robot-docs",
            ].map(|verb| format!("\"{}\"", escape_json(verb))).join(",");
            let reliance_verbs = reliance::VERBS_JSON;
            let reliance_docs = reliance::ROBOT_DOCS_JSON;
            let track_record_modes = concat!(
                "{\"mode\":\"retrospective_descriptive\",\"schema\":\"fmine.guidance.track-record.v1\",",
                "\"input\":\"Hash-verified registered workbooks and agreed document transcriptions\",",
                "\"outcome_selector\":\"Original annual releases and comparative-only populations remain separate\",",
                "\"price_sensitivity\":\"Paired AISC sensitivity on complete declared quarterly evidence, exact proportionality and current-period compatibility; --price-subject explicitly selects a commodity-price proxy\",",
                "\"as_of\":{\"mode\":\"descriptive_source_comparison\",\"schema\":\"fmine.guidance.track-record.as-of.v1\",",
                "\"cut\":\"Explicit --as-of YYYY-MM-DD; no inferred wall clock; exclusive with --through and --price-subject\",",
                "\"price_sensitivity\":\"Unconditional scores only; --price-subject is rejected\",",
                "\"population\":\"Earliest registered agreed guidance per subject, metric, fiscal year and ownership basis; revisions remain visible\",",
                "\"outcomes\":\"First-published and latest registered assessments retain missing members\"},",
                "\"scope\":\"Registered transcription population; no completeness, historical forecast registration, ranking or calibration verdict\"}"
            );
            let mut payload = format!(
                "{{\"binary\":\"fmine\",\"envelope\":\"FMT-ENV-1\",\"envelope_fields\":{ENVELOPE_FIELDS},\"verbs\":[{verbs},{reliance_verbs}],\"issuers\":[{}],\"issuer_registry\":\"registries/sources/<issuer>.toml\",\"exit_codes\":{{\"0\":\"ok; lint verdicts and track-record misses are data\",\"2\":\"usage error (unknown verb, flag, issuer or selection); the envelope still prints\"}},\"claim_inspection\":{{\"lookup\":\"Exact canonical IDs from the synthetic run or hash-verified registered sources; explicit selectors bind the ledger snapshot and claims digest\",\"why\":\"Recorded evidence, interpretation, computed color and Claim admission guards; absent assessments remain absent\",\"trace\":\"Available source anchors and registered provenance; unresolved parents are explicit\"}},\"lint\":{{\"mode\":\"cited equality by default; --discover searches uncited equalities\",\"link\":\"[printed number and unit](fmine:clm_ID)\",\"predicate\":\"One explicit subject, reporting period, ownership basis, supported metric, unit and equality per sentence or line; other prose is unchecked\",\"discovery\":\"All semantic candidates remain visible; multiple matches stay ambiguous and the amount never selects a vintage\",\"scope\":\"Selected registered source snapshot; no publication-time or disclosure-permission verdict\"}},\"forecast_assessment\":{{\"mode\":\"read_only_replay\",\"input\":\"Ordered forecast, cohort, outcome and assessment TOML rows\",\"default_selector\":\"first-published\",\"provenance\":{{\"schema\":\"fmine.forecast-provenance.v1\",\"tables\":[\"node\",\"anchor\",\"binding\"],\"cut\":\"Each frozen forecast registered_at, not the assessment date\",\"coverage\":\"Every cohort member, including unresolved outcomes\",\"without_bundle\":\"not_checked\",\"generator_replay\":\"not_performed\",\"example\":\"tests/forecasts/provenance_synthetic.toml\"}},\"times_and_citations\":\"Supplied records, not external time proof or independently verified sources\"}},\"reliance\":{reliance_docs},\"track_record\":{track_record_modes},\"refusals\":[{}]}}",
                issuers.join(","),
                refusals.join(",")
            );
            payload.pop();
            payload.push_str(concat!(
                ",\"reliance_explicit\":{\"clock\":\"Caller-supplied --now UTC and --uncertainty-ns; no host clock inference\",",
                "\"trust\":\"Independent final head required for current reuse; historical --as-of never authorizes reuse\",",
                "\"identity\":\"assure issue binds the build-time revision, dirty marker and BLAKE3 of the running Linux executable\",",
                "\"compatibility\":\"Shares the report metadata and journal with reliance issue; grammars cannot be mixed\",",
                "\"recall\":\"Known issuances and generic roles only; retrieval registers no new recipient and sends no messages\"}}"
            ));
            Envelope::ok(
                "robot-docs",
                payload,
                vec![NextAction::new("fmine doctor", DOCTOR_WHY)],
            )
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClaimView {
    Show,
    Why,
    Trace,
}

impl ClaimView {
    const fn verb(self) -> &'static str {
        match self {
            Self::Show => "claim show",
            Self::Why => "claim why",
            Self::Trace => "claim trace",
        }
    }
}

fn claim_actions(id: &str, view: ClaimView, selector: &str) -> Vec<NextAction> {
    let mut actions = Vec::new();
    for (next, why) in [
        (
            ClaimView::Show,
            "the canonical value, semantic key and source citation",
        ),
        (
            ClaimView::Why,
            "the recorded evidence and Claim admission guards",
        ),
        (
            ClaimView::Trace,
            "the source anchors and available provenance",
        ),
    ] {
        if next != view {
            actions.push(NextAction::new(
                format!("fmine {} {id} {selector}", next.verb()),
                why,
            ));
        }
    }
    actions.push(NextAction::new(
        format!("fmine assure public {selector}"),
        "the verified source snapshot containing these public Claims",
    ));
    actions
}

/// DESIGN (BD-332, BD-115): all three views resolve the same canonical Claim. Synthetic IDs must match an
/// existing fixture Claim exactly; registered public IDs still require verified source bytes. Mine Stats
/// uses its direct reader and AISC the existing assurance projection, without a parallel Claim store.
fn inspect_public_claim(id: &str, root: &Path, view: ClaimView) -> Envelope {
    let verb = view.verb();
    let actions = || vec![NextAction::new("fmine doctor", DOCTOR_WHY)];
    let found = |cell: &fmine_public::key::KeyedCell,
                 profile: &fmine_public::sources::IssuerProfile,
                 built_in: bool| {
        let payload = match view {
            ClaimView::Show => cell.to_json(),
            ClaimView::Why => fmine_public::key::inspect::why_json(cell),
            ClaimView::Trace => fmine_public::key::inspect::trace_json(cell, profile),
        };
        let selector = if built_in {
            "--synthetic".into()
        } else {
            format!("--issuer {}", profile.issuer)
        };
        Envelope::ok(verb, payload, claim_actions(id, view, &selector))
    };
    if !id.starts_with("clm_") || id.parse::<fmine_core::id::ObjectId>().is_err() {
        return Envelope::refused(
            verb,
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("invalid public claim id '{id}'"),
            ),
            actions(),
        );
    }
    if let Ok(report) = run_synthetic_assurance(&[])
        && let Some(cell) = report
            .public_claims
            .cells
            .values()
            .find(|cell| cell.claim.claim_id.to_string() == id)
    {
        return found(cell, &fmine_public::synthetic::synthetic_profile(), true);
    }
    let mut input_errors = Vec::new();
    let mut source_failure = None;
    for issuer in registered_issuers(root) {
        let profile = match load_issuer_profile(root, &issuer) {
            Ok(profile) => profile,
            Err(refusal) => {
                if source_failure.is_none() {
                    source_failure = Some(refusal);
                }
                continue;
            }
        };
        let claims = fmine_public::key::mine_stats::load_claims(&profile, root);
        match claims {
            Ok(claims) => {
                if let Some(c) = claims
                    .claims
                    .iter()
                    .find(|c| c.claim.claim_id.to_string() == id)
                {
                    return found(c, &profile, false);
                }
                input_errors.extend(
                    claims
                        .refusals
                        .into_iter()
                        .map(|r| format!("{}: {}", r.locator, r.reason)),
                );
                match run_public_assurance(&profile, root) {
                    Ok(report) => {
                        if let Some(c) = report
                            .public_claims
                            .cells
                            .values()
                            .find(|c| c.claim.claim_id.to_string() == id)
                        {
                            return found(c, &profile, false);
                        }
                    }
                    Err(refusal) if source_failure.is_none() => source_failure = Some(refusal),
                    Err(_) => {}
                }
            }
            Err(refusal) => {
                if source_failure.is_none() {
                    source_failure = Some(refusal);
                }
            }
        }
    }
    if let Some(refusal) = source_failure {
        return Envelope::refused(verb, refusal, actions());
    }
    let detail = if input_errors.is_empty() {
        String::new()
    } else {
        format!("; unreadable claim inputs: {}", input_errors.join("; "))
    };
    Envelope::refused(
        verb,
        Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!("public claim '{id}' is absent from the registered public sources{detail}"),
        ),
        actions(),
    )
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
    fn reliance_router_preserves_required_values_before_global_options() {
        let mut issue = args(&[
            "assure",
            "issue",
            "--synthetic",
            "--output",
            "report.json",
            "--journal",
            "journal",
            "--expected-head",
            &"a".repeat(64),
            "--key",
            "owner.key",
            "--role",
            "sponsor",
            "--purpose",
            "review",
            "--now",
            "2026-11-01T00:00:00Z",
            "--uncertainty-ns",
            "0",
        ]);
        for (flag, inserted) in [
            ("--purpose", vec!["--json"]),
            ("--purpose", vec!["--format", "json"]),
            ("--key", vec!["--workspace", "/other"]),
        ] {
            let mut malformed = issue.clone();
            let value = malformed.iter().position(|arg| arg == flag).unwrap() + 1;
            malformed.splice(value..value, inserted.into_iter().map(str::to_string));
            let error = parse_args(&malformed, Path::new(".")).unwrap_err();
            assert!(
                error.contains(&format!("missing value for {flag}")),
                "{error}"
            );
        }
        issue.splice(0..0, args(&["--workspace", "/selected"]));
        issue.extend(args(&["--format", "markdown"]));
        let (command, format, workspace) = parse_args(&issue, Path::new(".")).unwrap();
        assert!(matches!(command, CliCommand::Issued(_)));
        assert_eq!(format, OutputFormat::Markdown);
        assert_eq!(workspace, Path::new("/selected"));
        for value in ["", "--json"] {
            assert!(
                parse_args(
                    &args(&["--workspace", value, "reliance", "check"]),
                    Path::new(".")
                )
                .is_err()
            );
        }
    }

    #[test]
    fn reliance_router_preserves_published_grammar_without_parser_fallback() {
        let digest = "a".repeat(64);
        let explicit = args(&[
            "--json",
            "reliance",
            "check",
            &digest,
            "--journal",
            "journal",
        ]);
        let host = args(&[
            "--json",
            "reliance",
            "check",
            "--digest",
            &digest,
            "--journal",
            "journal",
        ]);
        assert!(matches!(
            parse_args(&explicit, Path::new(".")).unwrap().0,
            CliCommand::RelianceExplicit(_)
        ));
        assert!(matches!(
            parse_args(&host, Path::new(".")).unwrap().0,
            CliCommand::Reliance(_)
        ));
        let mut required = host.clone();
        required.insert(3, "--require-current".into());
        assert!(matches!(
            parse_args(&required, Path::new(".")).unwrap().0,
            CliCommand::Reliance(_)
        ));
        let mut mixed = host;
        mixed.extend(args(&[
            "--now",
            "2026-11-01T00:00:00Z",
            "--uncertainty-ns",
            "0",
        ]));
        assert!(parse_args(&mixed, Path::new(".")).is_err());
    }

    #[test]
    fn forecast_assessment_selectors_and_time_resolution_are_explicit() {
        let base = [
            "forecast",
            "assess",
            "--ledger",
            "tests/forecasts/cohort_synthetic.toml",
            "--cohort",
            "synthetic-demo",
            "--as-of",
            "2026-10-31T00:00:00Z",
        ];
        let (command, _, _) = parse_args(&args(&base), &workspace_root()).expect("assessment args");
        assert!(
            matches!(command, CliCommand::ForecastAssess { selector, .. } if selector == "first-published")
        );
        let mut latest = args(&base);
        latest.extend(args(&["--selector", "latest-as-of"]));
        let (command, _, _) = parse_args(&latest, &workspace_root()).expect("restated selection");
        assert!(
            matches!(command, CliCommand::ForecastAssess { selector, .. } if selector == "latest-as-of")
        );
        let mut guarded = latest;
        guarded.extend(args(&[
            "--provenance",
            "tests/forecasts/provenance_synthetic.toml",
        ]));
        let (command, _, _) = parse_args(&guarded, &workspace_root()).expect("provenance selector");
        assert!(
            matches!(command, CliCommand::ForecastAssess { provenance: Some(path), .. }
            if path == Path::new("tests/forecasts/provenance_synthetic.toml"))
        );
        for bad in [
            vec!["forecast"],
            vec!["forecast", "freeze"],
            vec!["forecast", "assess"],
            vec![
                "forecast",
                "assess",
                "--ledger",
                "cohort.toml",
                "--cohort",
                "demo",
                "--as-of",
                "2026-10-31",
            ],
            vec![
                "forecast",
                "assess",
                "--ledger",
                "cohort.toml",
                "--cohort",
                "demo",
                "--as-of",
                "2026-10-31T00:00:00Z",
                "--selector",
                "latest",
            ],
            vec!["forecast", "assess", "--synthetic"],
            vec!["forecast", "assess", "--issuer", "barrick"],
        ] {
            let (envelope, _) = invoke(&args(&bad), &workspace_root());
            assert_eq!(
                (envelope.verb.as_str(), envelope.exit_code),
                ("forecast assess", 2),
                "{bad:?}"
            );
        }
        let (envelope, _) = invoke(
            &args(&[
                "guidance",
                "completion",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
                "--through",
                "2026Q2",
                "--as-of",
                "2026-10-31T00:00:00Z",
            ]),
            &workspace_root(),
        );
        assert_eq!(
            envelope.exit_code, 2,
            "guidance keeps its calendar-date contract"
        );
    }

    #[test]
    fn forecast_assessment_checks_all_supplied_provenance_without_issuing_a_certificate() {
        let selected = args(&[
            "forecast",
            "assess",
            "--ledger",
            "tests/forecasts/cohort_synthetic.toml",
            "--cohort",
            "synthetic-demo",
            "--as-of",
            "2026-10-31T00:00:00Z",
            "--provenance",
            "tests/forecasts/provenance_synthetic.toml",
        ]);
        let (env, _) = invoke(&selected, &workspace_root());
        assert_eq!(env.status, "ok", "{}", env.to_json());
        assert_eq!(env.exit_code, 0);
        assert!(env.anchor_id.is_none() && env.certificate.is_none());
        assert!(
            env.payload_json
                .contains("\"temporal_validation\":{\"status\":\"checked\"")
        );
        assert_eq!(env.payload_json.matches("\"cone_digest\"").count(), 3);
        assert!(
            env.payload_json
                .contains("\"n_members\":3,\"n_resolved\":2,\"n_unresolved\":1")
        );
        assert!(
            env.payload_json
                .contains("\"generator_replay\":\"not_performed\"")
        );
        assert_eq!(
            env.to_json(),
            invoke(&selected, &workspace_root()).0.to_json()
        );
        let (docs, _) = invoke(&args(&["robot-docs"]), &workspace_root());
        assert!(docs.payload_json.contains("[--provenance <path>]"));
        assert!(docs.payload_json.contains("fmine.forecast-provenance.v1"));
        for bad in [
            args(&["doctor", "--provenance", "unused.toml"]),
            args(&["forecast", "assess", "--provenance"]),
        ] {
            assert_eq!(invoke(&bad, &workspace_root()).0.exit_code, 2);
        }
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
        // bd-jg43.3: column G scored on displayed values, column N "Site G" refused by format.
        assert!(
            json.contains("\"totals\":{\"i\":40,\"ii\":2,\"iii\":1,\"iv\":2,\"v\":5,\"np\":5}")
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

    #[test]
    fn test_challenge_command_renders_cited_json_and_readable_markdown() {
        let query = args(&["assure", "challenges", "--synthetic"]);
        let (env, _) = invoke(&query, &workspace_root());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "assure challenges");
        assert!(
            env.payload_json
                .contains("\"schema\":\"fmine.challenge_packets.v1\"")
        );
        assert!(env.payload_json.contains("\"subject\":\"Site B\""));
        assert!(env.payload_json.contains("\"component_sum\":{\"v\":250,"));
        assert!(env.payload_json.contains("\"sign_flip_witness\":\"C6\""));
        assert!(env.payload_json.contains("\"external_use_allowed\":false"));
        assert!(env.to_json().contains(FLAGS));
        assert_eq!(invoke(&query, &workspace_root()).0.to_json(), env.to_json());
        let (markdown, exit) = run(
            &args(&[
                "assure",
                "challenges",
                "--synthetic",
                "--format",
                "markdown",
            ]),
            &workspace_root(),
        );
        assert_eq!(exit, 0);
        assert!(markdown.starts_with("# Evidence challenge packets\n"));
        assert!(markdown.contains("## Closing evidence requested"));
        assert!(markdown.contains("## Source operands"));
        assert!(markdown.contains("invented synthetic workbooks"));
        assert!(!markdown.contains("external_use_allowed\":true"));
    }

    #[test]
    fn test_guidance_command_is_exact_cited_and_explicitly_conditional() {
        let query = args(&["guidance", "completion", "--synthetic"]);
        let (env, _) = invoke(&query, &workspace_root());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "guidance completion");
        assert!(env.payload_json.contains("\"lane\":\"synthetic\""));
        assert!(env.payload_json.contains("\"envelope_tier\":\"T1\""));
        assert!(
            env.payload_json
                .contains("within_supplied_history_under_sales_scenario")
        );
        assert!(env.payload_json.contains("\"independently_checked\":true"));
        // Invented target 200..230 minus actual 68 rounded to the nearest koz gives [131.5,162.5].
        assert!(
            env.payload_json
                .contains("\"lo\":\"263/2\",\"hi\":\"325/2\"")
        );
        assert!(
            env.payload_json
                .contains("ASSUMPTION: repeat preceding-year")
        );
        assert!(env.payload_json.contains("\"source_claims\":[{"));
        assert!(
            env.anchor_id.is_none(),
            "a computation is not a registered forecast"
        );
        assert!(
            env.certificate.is_none(),
            "a content digest is not a ledger root"
        );
        let digest = fmine_public::guidance::run_synthetic()
            .expect("synthetic guidance")
            .digest
            .to_hex();
        assert!(
            env.payload_json
                .contains(&format!("\"digest\":\"{digest}\""))
        );
        assert_eq!(env.no_claim, Some(fmine_public::guidance::NO_CLAIM));
        assert_eq!(invoke(&query, &workspace_root()).0, env);
        let (markdown, exit) = run(
            &args(&[
                "guidance",
                "completion",
                "--synthetic",
                "--format",
                "markdown",
            ]),
            &workspace_root(),
        );
        assert_eq!(exit, 0);
        assert!(markdown.starts_with("# Guidance completion:"));
        assert!(markdown.contains("Invented synthetic evidence"));
        assert!(markdown.contains("131.5 to 162.5 koz"));
        assert!(markdown.contains("Checked witness"));
        assert!(markdown.contains("## Source citations"));
        assert!(markdown.contains("conditional on that sales interval"));
        assert!(markdown.contains("not a mine-capacity conclusion or a forecast"));
        assert!(!markdown.starts_with("```json"));
    }

    #[test]
    fn test_track_record_as_of_envelope_preserves_original_outcomes_and_missing_members() {
        let report = fmine_public::guidance::track_record::synthetic;
        let env = track_record_envelope(report());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "track-record");
        for required in [
            "\"schema\":\"fmine.guidance.track-record.as-of.v1\"",
            "\"lane\":\"synthetic\"",
            "\"gate_eligible\":false",
            "\"n_members\":2,\"n_scored\":1,\"n_unresolved\":1,\"n_inside\":0",
            "\"n_members\":2,\"n_scored\":1,\"n_unresolved\":1,\"n_inside\":1",
            "\"z\":\"3/2\"",
            "\"z\":\"1/2\"",
            "\"cp90_iid\":{",
            "\"whole_cohort_bounds\":{\"lo\":\"0\",\"hi\":\"1/2\"",
            "Synthetic_2030Q4.xlsx:YTD!C4",
            "Synthetic_2031Q4.xlsx:YTD!C4",
            "historical completeness unassessed",
            "Cost scores are unconditional",
        ] {
            assert!(
                env.payload_json.contains(required),
                "missing {required}: {}",
                env.payload_json
            );
        }
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        assert_eq!(
            env.no_claim,
            Some(fmine_public::guidance::track_record::NO_CLAIM)
        );
        assert!(env.to_json().contains(FLAGS));
        assert_eq!(track_record_envelope(report()), env);
        let markdown = env
            .rendered_markdown
            .as_deref()
            .expect("readable as-of report");
        assert!(markdown.starts_with("# Guidance track record"));
        assert!(markdown.contains("125; above; z 3/2"));
        assert!(markdown.contains("115; inside; z 1/2"));
        assert!(markdown.contains("1 unresolved of 2 members"));
        assert!(markdown.contains("## Sources"));
        assert!(markdown.contains("Synthetic invented evidence"));
    }

    #[test]
    fn test_track_record_requires_an_explicit_evidence_cut_and_registered_inputs() {
        let (command, _, _) = parse_args(
            &args(&[
                "track-record",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
                "--as-of",
                "2026-11-01",
            ]),
            &workspace_root(),
        )
        .expect("explicit evidence cut");
        assert_eq!(
            command,
            CliCommand::TrackRecordAsOf {
                issuer: "barrick".into(),
                site: Some("north-mara".into()),
                as_of: "2026-11-01".into(),
            }
        );
        let (mixed, _) = invoke(
            &args(&[
                "--price-subject",
                "Barrick",
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
            ]),
            &workspace_root(),
        );
        assert_eq!((mixed.verb.as_str(), mixed.exit_code), ("track-record", 2));
        assert_eq!(mixed.status, "usage_error");
        assert!(mixed.refusal.is_none());
        assert!(
            mixed
                .payload_json
                .contains("--as-of cannot be combined with --price-subject")
        );
        for bad in [
            vec!["track-record"],
            vec!["track-record", "--as-of", "2026-11-01"],
            vec![
                "track-record",
                "--issuer",
                "unknown",
                "--as-of",
                "2026-11-01",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-02-30",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01T00:00:00Z",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
                "--site",
                "",
            ],
            vec![
                "track-record",
                "--issuer",
                "barrick",
                "--through",
                "2025Q4",
                "--as-of",
                "2026-11-01",
            ],
            vec!["track-record", "--synthetic", "--issuer", "barrick"],
            vec!["track-record", "--synthetic", "--as-of", "2026-11-01"],
            vec!["track-record", "--synthetic", "--price-subject", "Barrick"],
            vec!["track-record", "--synthetic", "--site", "north-mara"],
            vec!["track-record", "--synthetic", "extra"],
        ] {
            let (env, _) = invoke(&args(&bad), &workspace_root());
            assert_eq!(env.exit_code, 2, "{bad:?}: {}", env.to_json());
            assert!(env.anchor_id.is_none());
            assert!(env.certificate.is_none());
        }
        let ws = no_corpus_workspace("track-record-no-inputs");
        let (env, _) = invoke(
            &args(&[
                "track-record",
                "--issuer",
                "barrick",
                "--as-of",
                "2026-11-01",
            ]),
            &ws,
        );
        assert_eq!(env.status, "refused");
        let refusal = env.refusal.expect("missing registered workbooks");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("Barrick_Q1_2024_Mine_Stats.xlsx"));
        assert!(refusal.what.contains("Barrick_Q2_2026_Mine_Stats.xlsx"));
        assert_eq!(env.payload_json, "null");
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        let (docs, _) = invoke(&args(&["robot-docs"]), &workspace_root());
        assert!(docs.payload_json.contains("track-record --issuer"));
        assert!(docs.payload_json.contains("track-record --synthetic"));
        assert!(docs.payload_json.contains("[--price-subject <subject>]"));
        assert!(
            docs.payload_json
                .contains("Unconditional scores only; --price-subject is rejected")
        );
    }

    #[test]
    fn test_recovery_command_preserves_models_citations_and_authority_boundary() {
        let query = args(&["recovery", "gap", "--synthetic"]);
        let (env, _) = invoke(&query, &workspace_root());
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        assert_eq!(env.verb, "recovery gap");
        for required in [
            "\"schema\":\"fmine.recovery.gap.v1\"",
            "\"lane\":\"synthetic\"",
            "\"constant_tails\":{",
            "\"grade_dependent_tails\":{",
            "\"gap_pp\":{\"point\":{",
            "\"source_claims\":[{",
            "\"training_inputs\":[{",
            "\"anchors\":[{",
            "\"descriptive_only\":true",
            "\"identifying\":false",
            "\"calibration\":\"uncalibrated_descriptive_model\"",
            "conservative input-rounding enclosures, not confidence or prediction intervals",
            "\"monetary_downside\":\"unquantified\"",
        ] {
            assert!(env.payload_json.contains(required), "missing {required}");
        }
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        assert_eq!(env.no_claim, Some(fmine_metrics::recovery::NO_CLAIM));
        assert!(env.to_json().contains(FLAGS));
        let digest = fmine_public::recovery::run_synthetic()
            .expect("synthetic recovery")
            .digest()
            .to_hex();
        assert!(
            env.payload_json
                .contains(&format!("\"digest\":\"{digest}\""))
        );
        assert_eq!(invoke(&query, &workspace_root()).0, env);
    }

    #[test]
    fn test_recovery_command_requires_evidence_and_unambiguous_selectors() {
        let ws = no_corpus_workspace("recovery-no-corpus");
        let (env, _) = invoke(&args(&["recovery", "gap", "--issuer", "barrick"]), &ws);
        assert_eq!(env.status, "refused");
        assert_eq!(env.refusal.expect("missing source").code, "FMINE-REF-506");
        assert_eq!(env.payload_json, "null");
        assert!(env.anchor_id.is_none());
        assert!(env.certificate.is_none());
        let (command, _, _) = parse_args(
            &args(&[
                "recovery",
                "gap",
                "--issuer",
                "barrick",
                "--through",
                "2025Q4",
            ]),
            &workspace_root(),
        )
        .expect("retrospective workbook selection");
        assert_eq!(
            command,
            CliCommand::RecoveryGap {
                issuer: "barrick".into(),
                through: Some("2025Q4".into()),
            }
        );
        for bad in [
            vec!["recovery"],
            vec!["recovery", "gap"],
            vec!["recovery", "unknown", "--synthetic"],
            vec!["recovery", "gap", "--issuer", "unknown"],
            vec!["recovery", "gap", "--synthetic", "--issuer", "barrick"],
            vec!["recovery", "gap", "--synthetic", "--through", "2025Q4"],
            vec![
                "recovery",
                "gap",
                "--issuer",
                "barrick",
                "--through",
                "2025Q5",
            ],
            vec!["recovery", "gap", "--synthetic", "--site", "Lumwana"],
            vec!["recovery", "gap", "--synthetic", "--as-of", "2026-10-31"],
            vec!["recovery", "gap", "--synthetic", "extra"],
        ] {
            let (env, _) = invoke(&args(&bad), &workspace_root());
            assert_eq!(env.exit_code, 2, "{bad:?}: {}", env.to_json());
            assert!(env.payload_json.starts_with("{\"error\":"));
            assert!(env.anchor_id.is_none());
            assert!(env.certificate.is_none());
        }
    }

    #[test]
    #[ignore = "requires verified Barrick workbooks, PDFs and local two-reader recovery inputs"]
    fn test_real_recovery_is_embedded_in_assurance_with_separate_digest() {
        let root = workspace_root();
        let recovery = fmine_public::recovery::run_public(&root, "barrick", None)
            .expect("registered recovery evidence");
        assert_eq!(recovery.quarters.len(), 14);
        assert!(recovery.quarters.iter().all(|q| q.outcome.is_ok()));
        let (env, _) = invoke(&args(&["assure", "public", "--issuer", "barrick"]), &root);
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{:?}", env.refusal);
        assert!(
            env.payload_json
                .contains(&format!("\"recovery\":{}", recovery.to_json()))
        );
        let roots = fmine_public::sources::load_expected_roots(
            &fmine_public::sources::expected_roots_path(&root, "barrick"),
        )
        .expect("existing identity/waterfall roots");
        let expected = roots.last().expect("current registered root");
        assert_eq!(
            env.certificate.as_deref(),
            Some(expected.state_root.as_str())
        );
        assert_eq!(env.anchor_id.as_deref(), Some(expected.anchor_id.as_str()));
        assert_ne!(recovery.digest().to_hex(), expected.state_root);
        assert!(env.to_json().contains(FLAGS));
    }

    #[test]
    fn test_new_commands_refuse_missing_evidence_and_reject_mixed_selectors() {
        let ws = no_corpus_workspace("new-commands-no-corpus");
        let (env, _) = invoke(&args(&["assure", "challenges", "--issuer", "barrick"]), &ws);
        assert_eq!(env.status, "refused");
        assert_eq!(env.refusal.expect("missing source").code, "FMINE-REF-506");
        assert_eq!(env.payload_json, "null");
        let (env, _) = invoke(
            &args(&[
                "guidance",
                "completion",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
                "--through",
                "2026Q2",
                "--as-of",
                "2026-10-31",
            ]),
            &ws,
        );
        assert_eq!(env.status, "refused");
        assert_eq!(env.refusal.expect("missing guidance").code, "FMINE-REF-201");
        assert_eq!(env.payload_json, "null");
        for bad in [
            vec!["assure", "challenges"],
            vec!["assure", "challenges", "--synthetic", "--issuer", "barrick"],
            vec![
                "guidance",
                "completion",
                "--synthetic",
                "--through",
                "2026Q2",
            ],
            vec![
                "guidance",
                "completion",
                "--issuer",
                "barrick",
                "--site",
                "north-mara",
            ],
            vec![
                "guidance",
                "completion",
                "--synthetic",
                "--as-of",
                "2026-02-30",
            ],
            vec!["doctor", "--site", "north-mara"],
        ] {
            assert!(
                parse_args(&args(&bad), &workspace_root()).is_err(),
                "{bad:?}"
            );
        }
    }

    const STATE_I: &str = "\"state\":\"i\"";
    const ISSUER_ASSERTION: &str = "\"evidence\":\"Assertion\",\"source\":\"issuer\"";

    /// The objects of every `"key":[{..},..]` array, with braces balanced outside strings.
    fn json_objects<'a>(json: &'a str, key: &str) -> Vec<&'a str> {
        let mut found = Vec::new();
        for (at, _) in json.match_indices(&format!("\"{key}\":[{{")) {
            let start = at + key.len() + 4;
            let (mut from, mut depth, mut in_str, mut escaped) = (start, 0_u32, false, false);
            for (i, c) in json[start..].char_indices() {
                match c {
                    _ if escaped => escaped = false,
                    '\\' if in_str => escaped = true,
                    '"' => in_str = !in_str,
                    '{' if !in_str => {
                        if depth == 0 {
                            from = start + i;
                        }
                        depth += 1;
                    }
                    '}' if !in_str => {
                        depth -= 1;
                        if depth == 0 {
                            found.push(&json[from..=start + i]);
                        }
                    }
                    ']' if !in_str && depth == 0 => break,
                    _ => {}
                }
            }
        }
        found
    }

    /// bd-306n.3: an issuer's figure is reported as an assertion, whether it reproduced or not. Reproducing shows
    /// its figures agree with each other; it does not show they are true.
    #[test]
    fn test_assure_synthetic_reports_assertion_issuer() {
        let root = workspace_root();
        let (cmd, _, ws) =
            parse_args(&args(&["assure", "public", "--synthetic"]), &root).expect("parse");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"));
        let json = env.to_json();
        let instances = json_objects(&json, "instances");
        // The walk must reach the 40 state-(i) instances, or the loop below passes on nothing.
        let reproduced = instances.iter().filter(|i| i.contains(STATE_I)).count();
        assert_eq!(reproduced, 40, "{json}");
        // 2 + 1 + 2 in the visible scope, and the one the hidden sheet `Q4 2029` holds.
        let exceptions = json_objects(&json, "exceptions");
        assert_eq!(exceptions.len(), 6, "{json}");
        for instance in instances.iter().chain(&exceptions) {
            assert!(instance.contains(ISSUER_ASSERTION), "{instance}");
        }
        println!(
            "{{\"test\":\"assure_synthetic_assertion_issuer\",\"instances\":{},\"exceptions\":{},\
             \"non_assertion\":0}}",
            instances.len(),
            exceptions.len()
        );
    }

    /// bd-306n.3 over the real workbooks: one line per workbook, then a summary. Run at the 2026Q2 bound, so a
    /// later quarter's registry row changes neither the file set nor E822's count.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded_docs"]
    fn test_assure_public_reports_every_barrick_instance_as_issuer_assertion() {
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
        assert_eq!(env.exit_code, 0, "{}", env.to_json());
        let json = env.to_json();
        let mut by_file: std::collections::BTreeMap<&str, (u32, u32)> =
            std::collections::BTreeMap::new();
        for instance in json_objects(&json, "instances") {
            let at = instance.strip_prefix("{\"at\":\"").expect("at");
            let file = at.split(':').next().expect("file");
            let row = by_file.entry(file).or_default();
            row.0 += 1;
            row.1 += u32::from(instance.contains(ISSUER_ASSERTION));
        }
        let (mut total, mut other) = (0, 0);
        for (file, (instances, assertion_issuer)) in &by_file {
            let other_class = instances - assertion_issuer;
            println!(
                "{{\"file\":\"{file}\",\"instances\":{instances},\"assertion_issuer\":{assertion_issuer},\
                 \"other_class\":{other_class}}}"
            );
            total += instances;
            other += other_class;
        }
        println!(
            "{{\"files\":{},\"instances\":{total},\"non_assertion\":{other}}}",
            by_file.len()
        );
        // Instances come from the registered AISC workbooks; Mine Stats files carry none.
        let want: std::collections::BTreeSet<String> = load_issuer_profile(&root, "barrick")
            .and_then(|profile| profile.through("2026Q2"))
            .expect("registry")
            .sources
            .into_iter()
            .filter(|s| s.kind == fmine_public::sources::SourceKind::AiscReconciliation)
            .map(|s| s.file_name)
            .collect();
        let got: std::collections::BTreeSet<String> =
            by_file.keys().map(|f| (*f).to_string()).collect();
        assert_eq!(got, want);
        // E822's count at the 2026Q2 bound.
        assert_eq!(total, 2595);
        assert_eq!(other, 0);
    }

    /// BD-274 over the real workbooks: the gate-eligible run's payload ends with the seven published cut-offs.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded_docs"]
    fn test_assure_public_issuer_run_carries_the_published_cutoffs() {
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "barrick",
            "--through",
            "2026Q2",
        ]);
        assert_eq!(env.exit_code, 0, "{}", env.to_json());
        let section = published_cutoffs(&workspace_root(), "barrick")
            .expect("reads")
            .expect("barrick rows");
        let head = &section[..section.find(",\"rows\"").expect("rows")];
        println!("{head}}}");
        assert!(
            env.payload_json
                .ends_with(&format!(",\"published_cutoffs\":{section}}}")),
            "{head}"
        );
        assert_eq!(section.matches("{\"id\":").count(), 7);
    }

    /// BD-274: an issuer run carries the workspace's published cut-offs for the registry's own issuer only. No
    /// registry is no section; a registry that does not read refuses (506) rather than dropping rows.
    #[test]
    fn test_published_cutoffs_for_the_registry_issuer_only() {
        let root = workspace_root();
        let section = published_cutoffs(&root, "barrick")
            .expect("reads")
            .expect("barrick rows");
        assert!(
            section.contains(
                "\"cases\":7,\"reproduced\":6,\"within_input_rounding\":1,\"conflict\":0,"
            ),
            "{section}"
        );
        assert_eq!(published_cutoffs(&root, "newmont").expect("reads"), None);

        let ws = std::env::temp_dir().join(format!("fmine-cli-cutoffs-{}", std::process::id()));
        let reg = ws.join(cutoff::CUTOFFS_REGISTRY);
        std::fs::create_dir_all(reg.parent().expect("dir")).expect("registry dir");
        assert_eq!(
            published_cutoffs(&ws, "barrick").expect("no registry"),
            None
        );
        let text = std::fs::read_to_string(root.join(cutoff::CUTOFFS_REGISTRY)).expect("registry");
        std::fs::write(
            &reg,
            text.replacen("published = \"5.66\"", "published = 5.66", 1),
        )
        .expect("write");
        let refusal = published_cutoffs(&ws, "barrick").expect_err("malformed row");
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(
            refusal.what.contains("published_cutoffs.toml"),
            "{}",
            refusal.what
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// The section is the payload's last field and leaves the anchor and certificate alone: it is not in the root.
    /// The synthetic lane never carries it.
    #[test]
    fn test_published_cutoffs_join_the_payload_outside_the_state_root() {
        let report = run_synthetic_assurance(&[]).expect("synthetic");
        let section = published_cutoffs(&workspace_root(), "barrick")
            .expect("reads")
            .expect("barrick rows");
        let (bare, with) = (
            report_envelope(&report, None),
            report_envelope(&report, Some(&section)),
        );
        assert_eq!(
            (&bare.anchor_id, &bare.certificate),
            (&with.anchor_id, &with.certificate)
        );
        let body = bare.payload_json.strip_suffix('}').expect("object");
        assert_eq!(
            with.payload_json,
            format!("{body},\"published_cutoffs\":{section}}}")
        );
        let synthetic = envelope_for(&["assure", "public", "--synthetic"]);
        assert_eq!(synthetic.exit_code, 0);
        assert!(!synthetic.payload_json.contains("published_cutoffs"));
    }

    /// Without the registered workbooks the gate-eligible run refuses with 506 (foundation I/O, exit 5) and
    /// names the files; it never substitutes stand-in content (BD-321).
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
        assert_eq!((env.exit_code, env.status), (5, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-506\""));
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
            &["claim", "why", "clm_0001", "extra"][..],
            &["claim", "trace"][..],
            &["claim", "invent", "clm_0001"][..],
            &[
                "claim",
                "why",
                "clm_0001",
                "--synthetic",
                "--issuer",
                "barrick",
            ][..],
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
    fn test_cli_claim_show_refuses_invalid_id() {
        for action in ["show", "why", "trace"] {
            let env = invoke(&args(&["claim", action, "clm_0001"]), &workspace_root()).0;
            assert_eq!((env.exit_code, env.status), (3, "refused"));
            assert_eq!(env.verb, format!("claim {action}"));
            let json = env.to_json();
            assert!(json.contains("\"code\":\"FMINE-REF-201\""), "{json}");
            assert!(json.contains("invalid public claim id"), "{json}");
        }
    }

    /// BD-115: hand-written inspection payloads around the independently tested canonical cell JSON.
    /// The synthetic source is not registered on disk and remains an assumption with an unbounded color.
    #[test]
    fn test_claim_show_why_trace_golden() {
        let report = run_synthetic_assurance(&[]).expect("synthetic report");
        let cell = report
            .public_claims
            .cells
            .get("Synthetic_AISC.xlsx:Q1 2030!B5")
            .expect("source cell");
        let id = cell.claim.claim_id.to_string();
        let canonical = cell.to_json();
        assert!(canonical.contains("\"stored\":\"500\",\"displayed\":\"500\""));
        let uu_refusal = "{\"code\":\"FMINE-REF-204\",\"exit_code\":3,\
            \"what\":\"Unbudgeted unknowns: a decision-facing claim carries no unknown-unknown (UU) budget (G-UU, INV-015): \
            claim 'cost of sales applicable to gold production' has epistemic status KnownUnknown(Parametric) but zero UuTerm scenarios\",\
            \"fix\":\"Attach an explicit UU term (an unmodelled budget or a flip margin).\"}";
        let why = format!(
            "{{\"schema\":\"fmine.claim.why.v1\",\"claim\":{canonical},\
             \"interpretation\":\"The source number and its displayed text are preserved; extraction does not verify its truth.\",\
             \"evidence_class\":\"Assumed\",\"epistemic_status\":\"KnownUnknown(Parametric)\",\"knowledge_state\":\"Unknown\",\
             \"representation\":\"Deterministic\",\"support\":\"PeriodAggregate\",\"additivity\":\"ExtensiveAdditive\",\"confidentiality\":\"Public\",\
             \"color\":{{\"status\":\"available\",\"value\":{{\"kind\":\"estimated\",\"estimator\":\"class:assumed\",\
             \"dispersion\":null,\"dispersion_status\":\"unbounded\"}},\"refusal\":null}},\
             \"uu_terms\":[],\"uu_guard\":{{\"status\":\"refused\",\"refusal\":{uu_refusal}}},\
             \"public_export_guard\":{{\"status\":\"passed\",\"refusal\":null}},\
             \"decision_admission\":{{\"scope\":\"Claim color and UU guards only\",\"claim_guards_pass\":false,\
             \"first_refusal\":{uu_refusal},\"decision_context\":null,\"flip_margin\":null}},\
             \"calibration\":{{\"failed\":false,\"assessment\":null}},\
             \"freshness\":{{\"recorded_stale\":false,\"assessment\":\"not_assessed\",\
             \"reason\":\"This source projection has no clock or dependency-invalidation replay.\"}}}}"
        );
        let trace = format!(
            "{{\"schema\":\"fmine.claim.trace.v1\",\"claim\":{canonical},\"scope\":\"source_leaf\",\
             \"complete_for_claim\":true,\"parents\":[],\"unresolved_parents\":[],\
             \"anchors\":[{{\"artifact_path\":\"Synthetic_AISC.xlsx\",\"sha256\":\"{}\",\
             \"locator\":\"Q1 2030!B5\",\"registered_source\":null}}],\"supersedes\":null,\"recorded_stale\":false,\
             \"boundary\":\"Source cells and their registered interpretation are the leaves of this public projection; no upstream measurement history or dependency replay is asserted.\"}}",
            cell.key.reporting_vintage.file_sha256,
        );
        for (action, want) in [("show", canonical), ("why", why), ("trace", trace)] {
            let argv = args(&["claim", action, &id]);
            let (env, format) = invoke(&argv, &workspace_root());
            assert_eq!(
                (env.status, env.exit_code, format),
                ("ok", 0, OutputFormat::Json)
            );
            assert_eq!(env.verb, format!("claim {action}"));
            assert_eq!(env.payload_json, want, "{action}");
            assert_eq!(env.to_json().matches(FLAGS).count(), 1);
            assert_eq!(env.certificate, None);
            assert!(
                env.next_actions
                    .iter()
                    .all(|a| resolves(&a.command, &workspace_root()))
            );
            assert!(
                env.next_actions
                    .iter()
                    .any(|a| a.command == "fmine assure public --synthetic")
            );
            assert_eq!(invoke(&argv, &workspace_root()).0.to_json(), env.to_json());
        }
    }

    #[test]
    fn test_claim_why_uses_recorded_guards_and_trace_keeps_unresolved_parents() {
        let report = run_synthetic_assurance(&[]).expect("synthetic report");
        let mut cell = report.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B5"].clone();
        cell.claim.uu_terms.push(fmine_claim::UuTerm {
            scenario_id: "synthetic-downside".into(),
            description: "Invented test scenario".into(),
            downside_exposure_usd_m: 2.5,
            trigger_indicator: "synthetic trigger".into(),
        });
        let why = fmine_public::key::inspect::why_json(&cell);
        assert!(why.contains("\"claim_guards_pass\":true"));
        assert!(why.contains("\"downside_exposure_usd_m\":2.5"));
        assert!(why.contains("synthetic-downside"));
        assert!(why.contains("\"decision_context\":null"));
        cell.claim.calibration_failed = true;
        cell.claim.stale = true;
        let why = fmine_public::key::inspect::why_json(&cell);
        assert!(why.contains("FMINE-REF-205"));
        assert!(why.contains("\"claim_guards_pass\":false"));
        assert!(why.contains("\"recorded_stale\":true"));
        assert!(why.contains("\"assessment\":\"not_assessed\""));
        let parent = fmine_core::id::ObjectId::from_canonical_bytes(
            fmine_core::id::PrefixKind::Claim,
            b"unresolved-synthetic-parent",
        );
        cell.claim.parents.push(parent);
        let trace = fmine_public::key::inspect::trace_json(
            &cell,
            &fmine_public::synthetic::synthetic_profile(),
        );
        assert!(trace.contains("\"scope\":\"partial_provenance\",\"complete_for_claim\":false"));
        assert!(trace.contains(&format!("\"unresolved_parents\":[\"{parent}\"]")));
    }

    #[test]
    fn test_claim_inspection_absent_id_is_not_replaced_by_a_synthetic_claim() {
        let missing = fmine_core::id::ObjectId::from_canonical_bytes(
            fmine_core::id::PrefixKind::Claim,
            b"absent-public-claim",
        )
        .to_string();
        let ws = scratch("claim-absent");
        for action in ["show", "why", "trace"] {
            let env = invoke(&args(&["claim", action, &missing]), &ws).0;
            assert_eq!((env.status, env.exit_code), ("refused", 3));
            assert_eq!(env.refusal.expect("refusal").code, "FMINE-REF-201");
            assert_eq!(env.payload_json, "null");
        }
    }

    #[test]
    fn test_cli_claim_show_resolves_aisc_and_refuses_altered_source() {
        let ws = synthetic_workspace("claim-show-aisc");
        let profile = load_issuer_profile(&ws, "synthetic").expect("registry");
        let report = run_public_assurance(&profile, &ws).expect("report");
        let source = profile
            .sources
            .iter()
            .find(|s| s.kind == fmine_public::sources::SourceKind::AiscReconciliation)
            .expect("AISC source");
        let claim = report
            .public_claims
            .cells
            .values()
            .find(|c| c.locator.starts_with(&format!("{}:", source.file_name)))
            .expect("emitted AISC claim");
        let id = claim.claim.claim_id.to_string();
        for action in ["show", "why", "trace"] {
            let env = invoke(&args(&["claim", action, &id]), &ws).0;
            assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
            if action == "show" {
                assert_eq!(env.payload_json, claim.to_json());
            }
            if action == "why" {
                assert!(env.payload_json.contains("\"claim_guards_pass\":false"));
                assert!(env.payload_json.contains("FMINE-REF-204"));
            }
            if action == "trace" {
                assert!(env.payload_json.contains("\"scope\":\"source_leaf\""));
                assert!(
                    env.payload_json
                        .contains(&format!("\"url\":\"{}\"", source.url))
                );
                assert!(!env.payload_json.contains("\"registered_source\":null"));
            }
            assert!(env.payload_json.contains("\"evidence\":\"synthetic\""));
            assert!(env.to_json().contains(FLAGS_JSON));
            assert!(env.next_actions.iter().all(|a| resolves(&a.command, &ws)));
        }

        let path = ws.join(&profile.corpus_dir).join(&source.file_name);
        let mut bytes = std::fs::read(&path).expect("source bytes");
        bytes.push(0);
        std::fs::write(&path, bytes).expect("alter the registered source");
        for action in ["show", "why", "trace"] {
            let refused = invoke(&args(&["claim", action, &id]), &ws).0;
            assert_eq!((refused.status, refused.exit_code), ("refused", 8));
            assert_eq!(refused.refusal.expect("refusal").code, "FMINE-REF-505");
            assert_eq!(refused.payload_json, "null");
        }
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks"]
    fn test_cli_claim_show_resolves_public_source_claim() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick").expect("registry");
        let claims = fmine_public::key::mine_stats::load_claims(&profile, &root).expect("claims");
        let first = claims
            .claims
            .iter()
            .find(|c| c.locator == "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23")
            .expect("E474 first");
        let env = execute_command(
            &CliCommand::ClaimShow {
                id: first.claim.claim_id.to_string(),
                issuer: None,
                through: None,
                synthetic: false,
            },
            &root,
        );
        assert_eq!((env.status, env.exit_code), ("ok", 0));
        assert_eq!(env.payload_json, first.to_json());
        assert!(env.to_json().contains("1479"));
        assert!(env.to_json().contains(FLAGS_JSON));
        let id = first.claim.claim_id.to_string();
        let why = invoke(&args(&["claim", "why", &id]), &root).0;
        assert_eq!((why.status, why.exit_code), ("ok", 0));
        assert!(why.payload_json.contains("Assertion { source: Issuer }"));
        assert!(why.payload_json.contains("\"knowledge_state\":\"Known\""));
        assert!(why.payload_json.contains("FMINE-REF-205"));
        assert!(!why.payload_json.contains("FMINE-REF-204"));
        assert!(why.payload_json.contains("\"claim_guards_pass\":false"));
        let trace = invoke(&args(&["claim", "trace", &id]), &root).0;
        assert_eq!((trace.status, trace.exit_code), ("ok", 0));
        assert!(trace.payload_json.contains("\"complete_for_claim\":true"));
        assert!(
            trace
                .payload_json
                .contains(&first.key.reporting_vintage.file_sha256)
        );
        let source = profile
            .sources
            .iter()
            .find(|s| s.file_name == "Barrick_Q1_2024_Mine_Stats.xlsx")
            .expect("registered source");
        assert!(trace.payload_json.contains(&source.url));
        assert!(trace.payload_json.contains("\"locator\":\"QTD!G23\""));
    }

    #[test]
    fn test_claim_show_reads_reported_claims_and_refuses_invented_ids() {
        let report = run_synthetic_assurance(&[]).expect("synthetic report");
        for is_numeric in [false, true] {
            let cell = report
                .public_claims
                .cells
                .values()
                .find(|c| {
                    matches!(c.claim.value, fmine_public::key::PublicValue::Number { .. })
                        == is_numeric
                })
                .expect("numeric and placeholder Claims");
            let id = cell.claim.claim_id.to_string();
            for action in ["show", "why", "trace"] {
                let query = args(&["claim", action, &id, "--synthetic"]);
                let (env, _) = invoke(&query, &workspace_root());
                assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
                assert!(env.payload_json.contains(&cell.to_json()));
                assert!(env.payload_json.contains("\"lane\":\"synthetic\""));
                assert_eq!(
                    env.certificate,
                    Some(report.ledger_anchor.state_root.to_hex())
                );
                assert!(env.payload_json.contains(
                    &fmine_public::key::claims_digest(report.public_claims.cells.values()).to_hex()
                ));
                if action != "show" {
                    assert!(env.payload_json.contains(&format!("\"{action}\":{{")));
                }
                assert!(env.to_json().contains(FLAGS));
                assert_eq!(
                    invoke(&query, &workspace_root()).0,
                    env,
                    "the same source snapshot reads byte-identically"
                );
                assert!(
                    env.next_actions
                        .iter()
                        .all(|a| a.command.ends_with("--synthetic"))
                );
            }
        }
        for action in ["show", "why", "trace"] {
            let (unknown, _) = invoke(
                &args(&["claim", action, "clm_invented", "--synthetic"]),
                &workspace_root(),
            );
            assert_eq!((unknown.exit_code, unknown.status), (3, "refused"));
            assert_eq!(unknown.payload_json, "null");
            assert!(
                unknown
                    .refusal
                    .expect("missing id refusal")
                    .what
                    .contains("absent")
            );
        }
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks"]
    fn test_claim_show_reads_real_mine_stats_and_aisc_claims() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick").expect("profile");
        let report = run_public_assurance(&profile, &root).expect("verified public report");
        for is_mine_stats in [false, true] {
            let cell = report
                .public_claims
                .cells
                .values()
                .find(|c| {
                    (c.key.line.label.starts_with("site:")
                        || c.key.line.label.starts_with("summary:"))
                        == is_mine_stats
                })
                .expect("both real-source adapters emit Claims");
            let id = cell.claim.claim_id.to_string();
            let (env, _) = invoke(&args(&["claim", "show", &id, "--issuer", "barrick"]), &root);
            assert_eq!(env.exit_code, 0, "{}", env.to_json());
            assert!(env.payload_json.contains(&cell.to_json()));
            assert!(env.payload_json.contains("\"lane\":\"public\""));
            assert_eq!(
                env.certificate,
                Some(report.ledger_anchor.state_root.to_hex())
            );
        }
    }

    #[test]
    fn test_claim_show_rechecks_registered_source_bytes() {
        let ws = synthetic_workspace("claim-source-integrity");
        let profile = load_issuer_profile(&ws, "synthetic").expect("registered synthetic source");
        let bounded = profile.clone().through("2030Q1").expect("bound");
        let report = run_public_assurance(&bounded, &ws).expect("verified report");
        let cell = report
            .public_claims
            .cells
            .values()
            .next()
            .expect("public claim");
        let id = cell.claim.claim_id.to_string();
        let later = profile
            .sources
            .iter()
            .find(|s| s.quarter == "2030Q2")
            .expect("later source");
        std::fs::write(
            ws.join(&profile.corpus_dir).join(&later.file_name),
            b"altered later source",
        )
        .expect("alter later scratch fixture");
        for action in ["show", "why", "trace"] {
            let query = args(&[
                "claim",
                action,
                &id,
                "--issuer",
                "synthetic",
                "--through",
                "2030Q1",
            ]);
            let (read, _) = invoke(&query, &ws);
            assert_eq!(read.exit_code, 0, "{}", read.to_json());
            assert!(read.payload_json.contains(&cell.to_json()));
            assert_eq!(
                read.certificate,
                Some(report.ledger_anchor.state_root.to_hex())
            );
            assert!(
                read.next_actions
                    .iter()
                    .all(|a| a.command.ends_with("--issuer synthetic --through 2030Q1"))
            );
            assert!(read.next_actions.iter().all(|a| resolves(&a.command, &ws)));
            let unbounded = invoke(&args(&["claim", action, &id, "--issuer", "synthetic"]), &ws).0;
            assert_eq!(
                unbounded.refusal.expect("later source mismatch").code,
                "FMINE-REF-505"
            );
            let wrong_lane = invoke(&args(&["claim", action, &id, "--synthetic"]), &ws).0;
            assert_eq!(
                wrong_lane.refusal.expect("id absent from fixture").code,
                "FMINE-REF-201"
            );
        }
        let path = ws
            .join(&profile.corpus_dir)
            .join(&bounded.sources[0].file_name);
        std::fs::write(path, b"altered registered source").expect("alter scratch fixture");
        for action in ["show", "why", "trace"] {
            let query = args(&[
                "claim",
                action,
                &id,
                "--issuer",
                "synthetic",
                "--through",
                "2030Q1",
            ]);
            let (refused, _) = invoke(&query, &ws);
            assert_eq!((refused.exit_code, refused.status), (8, "refused"));
            assert_eq!(
                refused.refusal.expect("digest refusal").code,
                "FMINE-REF-505"
            );
            assert_eq!(refused.payload_json, "null");
        }
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
        // Regression pin of this implementation's output, not an independent value: the root and anchor of the
        // last row of repro/barrick/expected.toml (BD-339.S1), which records why each root moved.
        let expected = fmine_public::sources::load_expected_roots(
            &fmine_public::sources::expected_roots_path(&root, "barrick"),
        )
        .expect("expected roots");
        let last = expected.last().expect("at least one row");
        assert!(
            json.contains(&format!("\"state_root\":\"{}\"", last.state_root)),
            "root differs from the expected row of {}",
            last.commit
        );
        assert_eq!(
            env.anchor_id.as_deref(),
            Some(last.anchor_id.as_str()),
            "anchor differs from the expected row of {}",
            last.commit
        );
        assert!(json.contains("\"aisc_tables\":34,\"instances\":2595"));
        // BD-331.S2 (census E828): the hidden scope is reported, labelled and never counted. BD-331.S4 (region
        // census, E840): its three regional rates move v → i twice and v → iv once.
        assert!(json.contains(
            "\"hidden_scope\":{\"scope\":\"sheet not displayed to readers\",\"counted\":false,\
             \"aisc_tables\":6,\"instances\":435,\"by_state\":{\"i\":406,\"ii\":0,\"iii\":1,\"iv\":18,\"v\":0,\"np\":10}"
        ));
        assert!(json.contains(
            // BD-331.S3 (census E826): 263 stated-share fits move v → i and 65 misses v → iv. bd-jg43.3 (census
            // E838): E823's 12 Loulo-Gounkoto instances move v → i on displayed values. BD-331.S4 (region census,
            // E840): of the 51 regional rates, 41 move v → i and 10 v → iv.
            "\"totals\":{\"i\":2436,\"ii\":3,\"iii\":4,\"iv\":92,\"v\":0,\"np\":60}"
        ));
        // The public-lane envelope: the root as certificate, the no-claim line, and its two next actions.
        assert_eq!(env.certificate.as_deref(), Some(last.state_root.as_str()));
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

    /// A synthetic issuer workspace that `doctor` can also load: every top-level catalogue registry copied in.
    fn doctor_workspace(tag: &str) -> PathBuf {
        let ws = synthetic_workspace(tag);
        let registries = workspace_root().join("registries");
        for entry in std::fs::read_dir(&registries).expect("registries dir") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "toml") {
                let name = path.file_name().expect("file name");
                std::fs::copy(&path, ws.join("registries").join(name))
                    .expect("copy catalogue registry");
            }
        }
        ws
    }

    /// BD-339.S2: doctor checks each registered file's size and sha256. Every synthetic file verifies; after one
    /// byte of one file is flipped, that file alone is present with `digest_ok: false`.
    #[test]
    fn test_doctor_reports_digest_per_file() {
        let ws = doctor_workspace("doctor-digest");
        let profile = load_issuer_profile(&ws, "synthetic").expect("synthetic registry");
        let n = profile.sources.len();
        let env = execute_command(&CliCommand::Doctor, &ws);
        let json = env.to_json();
        assert!(
            json.contains(&format!(
                "{{\"issuer\":\"synthetic\",\"registry_ok\":true,\"sources_registered\":{n},\"sources_present\":{n},\"sources_verified\":{n},"
            )),
            "{json}"
        );
        let first = &profile.sources[0];
        let path = ws.join(&profile.corpus_dir).join(&first.file_name);
        let mut bytes = std::fs::read(&path).expect("synthetic file");
        bytes[0] ^= 1;
        std::fs::write(&path, &bytes).expect("flip one byte");
        let json = execute_command(&CliCommand::Doctor, &ws).to_json();
        assert!(
            json.contains(&format!(
                "\"sources_present\":{n},\"sources_verified\":{},",
                n - 1
            )),
            "{json}"
        );
        assert!(
            json.contains(&format!(
                "{{\"file\":\"{}\",\"present\":true,\"bytes_ok\":true,\"digest_ok\":false}}",
                first.file_name
            )),
            "{json}"
        );
        let missing = &profile.sources[1];
        std::fs::rename(
            ws.join(&profile.corpus_dir).join(&missing.file_name),
            ws.join(&profile.corpus_dir).join("moved_aside"),
        )
        .expect("move a scratch file aside");
        let json = execute_command(&CliCommand::Doctor, &ws).to_json();
        assert!(
            json.contains(&format!(
                "{{\"file\":\"{}\",\"present\":false,\"bytes_ok\":false,\"digest_ok\":false}}",
                missing.file_name
            )),
            "{json}"
        );
    }

    /// BD-339.S2: `repro/barrick/fetch.sh` reads the registry with its own awk parser, so that a reader can
    /// fetch without building. Its listing must equal the Rust loader's rows (file, url, bytes, sha256) in
    /// order: the registry is the one copy, and the two readers of it cannot drift apart unnoticed.
    #[test]
    fn test_fetch_list_equals_registry() {
        let root = workspace_root();
        let out = std::process::Command::new("bash")
            .arg(root.join("repro").join("barrick").join("fetch.sh"))
            .arg("--list")
            .env("FMINE_WORKSPACE", &root)
            .output()
            .expect("run fetch.sh --list");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let want: String = load_issuer_profile(&root, "barrick")
            .expect("registry")
            .sources
            .iter()
            .map(|s| {
                format!(
                    "{}\t{}\t{}\t{}\n",
                    s.file_name, s.url, s.bytes, s.sha256_hex
                )
            })
            .collect();
        assert_eq!(String::from_utf8_lossy(&out.stdout), want);
    }

    /// BD-339.S2: a digest mismatch is a diagnosis, not a refusal.
    #[test]
    fn test_doctor_never_refuses_on_digest_mismatch() {
        let ws = doctor_workspace("doctor-norefuse");
        let profile = load_issuer_profile(&ws, "synthetic").expect("synthetic registry");
        let path = ws
            .join(&profile.corpus_dir)
            .join(&profile.sources[0].file_name);
        std::fs::write(&path, b"not the registered bytes").expect("overwrite scratch file");
        let env = execute_command(&CliCommand::Doctor, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        assert!(
            env.to_json()
                .contains("\"present\":true,\"bytes_ok\":false,\"digest_ok\":false"),
            "{}",
            env.to_json()
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
        assert!(rob.contains("{\"code\":\"FMINE-REF-502\",\"exit_code\":6,\"status\":\"active\","));
        assert!(
            rob.contains("{\"code\":\"FMINE-REF-506\",\"exit_code\":5,\"status\":\"proposed\",")
        );
        assert_eq!(
            rob.matches("{\"code\":\"FMINE-REF-").count(),
            CATALOG.len(),
            "{rob}"
        );
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

    /// bd-plx1's e2e at the envelope: a synthetic workspace with one registered document and a two-reader
    /// transcription registry runs and prints the `transcriptions` section; one reader's altered token refuses
    /// (exit 8) naming the locator; a removed document refuses (exit 8) naming the file. Rows are invented.
    #[test]
    fn test_assure_public_transcriptions_refuse_by_locator_and_document() {
        let ws = synthetic_workspace("transcribe");
        let doc: &[u8] = b"%PDF-1.7 synthetic document, never parsed";
        std::fs::write(ws.join("corpus").join("Doc.pdf"), doc).expect("document");
        let sha = fmine_core::hash::sha256(doc).to_hex();
        let reg = registry_path(&ws, "synthetic");
        let mut toml = std::fs::read_to_string(&reg).expect("registry");
        toml.push_str(&format!(
            "\n[[document]]\nfile = \"Doc.pdf\"\nkind = \"mda\"\nurl = \"https://example.invalid/Doc.pdf\"\n\
             url_basis = \"W1 table\"\npublished_at = \"2030-02-01\"\npublished_at_basis = \"FACT: synthetic\"\n\
             pages = 3\nbytes = {}\nsha256 = \"{sha}\"\n",
            doc.len()
        ));
        std::fs::write(&reg, toml).expect("registry");
        let rows = |second: &str| {
            let entry = |reader: &str, token: &str| {
                format!(
                    "\n[[entry]]\ndoc = \"Doc.pdf\"\ndoc_sha256 = \"{sha}\"\npage = 2\ntable = \"Table\"\nrow = \"Row\"\n\
                     col = \"Col\"\ntoken = \"{token}\"\nquantity = \"q\"\nunit = \"Moz\"\nbasis = \"attributable\"\n\
                     period = \"2030\"\nreader = \"{reader}\"\n"
                )
            };
            format!(
                "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n{}{}",
                entry("reader_a", "(1.5)"),
                entry("reader_b", second)
            )
        };
        let path = fmine_public::sources::transcriptions_path(&ws, "synthetic");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        let cmd = CliCommand::AssurePublic {
            issuer: "synthetic".to_string(),
            through: None,
        };
        std::fs::write(&path, rows("(1.5)")).expect("rows");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
        assert!(
            env.to_json()
                .contains("\"entries\":2,\"accepted\":1,\"accepted_digest\":"),
            "{}",
            env.to_json()
        );
        // The readers disagree, so the value is not observable from the evidence (202, exit 3).
        std::fs::write(&path, rows("(1.6)")).expect("rows");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (3, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-202\""), "{json}");
        assert!(
            json.contains("at Doc.pdf:p2:Table:Row/Col (reader_a, reader_b: token)"),
            "{json}"
        );
        assert!(
            !json.contains("1.6") && json.contains("\"payload\":null"),
            "{json}"
        );
        std::fs::write(&path, rows("(1.5)")).expect("rows");
        // A registered document that is altered is a digest mismatch (505, exit 8); one that is gone is
        // foundation I/O (506, exit 5).
        std::fs::write(ws.join("corpus").join("Doc.pdf"), b"%PDF-1.4 altered")
            .expect("alter document");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (8, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-505\""), "{json}");
        std::fs::remove_file(ws.join("corpus").join("Doc.pdf")).expect("remove document");
        let env = execute_command(&cmd, &ws);
        assert_eq!((env.exit_code, env.status), (5, "refused"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-506\""), "{json}");
        assert!(json.contains("missing: Doc.pdf"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
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
            ("claim why", envelope_for(&["claim", "why", "clm_0001"])),
            ("claim trace", envelope_for(&["claim", "trace", "clm_0001"])),
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
    /// a scratch path; invalid claim IDs are checked in test_cli_claim_show_refuses_invalid_id). The
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
        let fix_506 =
            "Put each named file at its registered path, or correct the registry entry named.";
        let fix_201 = "Query a domain that has claims, or produce the claim first (a public figure is emitted \
                       by its report).";
        let expected = [
            format!(
                "{}\"certificate\":\"<certificate>\",\"warnings\":[],\"next_actions\":[{barrick_run},\
                 {robot_docs}],{no_claim}\"payload\":\"<payload>\",\"refusal\":null}}",
                head("assure public", "ok", 0, "\"<anchor>\"")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-506\",\"exit_code\":5,\"what\":\"<what>\",\"fix\":\"{fix_506}\"}}}}",
                head("assure public", "refused", 5, "null")
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
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim show", "refused", 3, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim why", "refused", 3, "null")
            ),
            format!(
                "{}\"certificate\":null,\"warnings\":[],\"next_actions\":[{doctor}],\"payload\":null,\
                 \"refusal\":{{\"code\":\"FMINE-REF-201\",\"exit_code\":3,\"what\":\"<what>\",\
                 \"fix\":\"{fix_201}\"}}}}",
                head("claim trace", "refused", 3, "null")
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
                Refusal::new(RefusalCode::NoClaimInDomain201, "absent"),
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
        assert_eq!(checked, 12);
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
            (&["claim", "why"][..], "claim why"),
            (&["claim", "trace"][..], "claim trace"),
            (&["claim", "--format", "json", "trace"][..], "claim trace"),
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
    fn test_refused_506_points_to_doctor() {
        let ws = no_corpus_workspace("refused-506");
        let env = envelope_for(&[
            "assure",
            "public",
            "--issuer",
            "barrick",
            "--workspace",
            &ws.display().to_string(),
        ]);
        assert_eq!(
            env.refusal.as_ref().map(|r| (r.code, r.exit_code)),
            Some(("FMINE-REF-506", 5))
        );
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
