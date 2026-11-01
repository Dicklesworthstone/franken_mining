//! FrankenMining CLI (`fmine`) implementation (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

mod claim_inspection;
mod decision;
mod forecast;
mod guidance;
mod guidance_capacity;
pub mod issued;
mod lint;
mod lint_guidance;
mod lint_publicness;
pub mod mirror;
pub mod packet;
pub mod reliance;
mod reliance_binding;
pub mod reliance_explicit;
mod tables;
mod track_record;
mod track_record_as_of;
pub mod track_record_binding;
mod track_record_data;
mod track_record_price;

pub use forecast::live as forecast_journal;
pub use forecast::record_score as forecast_record;

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
    /// Compare explicit monetary alternatives on one declared finite scenario set (BD-225).
    DecideCard { model: PathBuf },
    /// Compare explicitly selected public JV production/sales assertions on a common dated perimeter.
    Mirror(mirror::Command),
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
    /// Save exact assurance bytes and register their recipient under release authority.
    AssureExport(reliance::Command),
    /// Prepare cited internal-review evidence challenges from an assurance snapshot (BD-349).
    AssureChallenges {
        /// `None` selects the explicitly requested synthetic lane.
        issuer: Option<String>,
        through: Option<String>,
    },
    /// Inspect workbook identities, or explicitly declared PDF sums and reserve roll-forwards.
    CheckTables {
        /// `None` selects the invented workbook known answers.
        issuer: Option<String>,
        through: Option<String>,
        /// A strict identity registry selects only its cited PDF documents and quantities.
        definitions: Option<PathBuf>,
        /// Every declared PDF must have been publicly available by this date; no members are dropped.
        as_of: Option<String>,
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
        /// Explicit commodity-price subject, evaluated only against evidence available at the cut.
        price_subject: Option<String>,
    },
    /// Separate publication cohorts with original outcomes at an explicit evidence-availability date.
    TrackRecordCohortAsOf {
        issuer: String,
        site: Option<String>,
        through: Option<String>,
        as_of: String,
    },
    /// Invented guidance, missing outcomes and restatements through the same cohort adapter.
    TrackRecordCohortAsOfSynthetic { as_of: String },
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
        /// Assess dated publication evidence independently of numeric equality (BD-345.S4).
        as_of: Option<String>,
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
    /// Register, resolve and assess forecasts in a durable journal using the declared host-clock policy.
    ForecastJournal(forecast_journal::Command),
    /// Score a time-proved frozen record against original outcomes and its registered baselines (BD-076).
    ForecastRecord(forecast_record::Command),
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
            | "--cohort-as-of"
            | "--price-subject"
            | "--ledger"
            | "--cohort"
            | "--selector"
            | "--provenance"
            | "--timestamp-head"
            | "--timestamp-receipt"
            | "--timestamp-roots"
            | "--timestamp-intermediates"
            | "--definitions"
            | "--capacity-document"
            | "--output"
            | "--journal"
            | "--head"
            | "--release-key"
            | "--recipient-role"
            | "--purpose"
            | "--code-revision"
            | "--artifact-format"
            | "--source-issuance" => i += 2,
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
                    Some("reissue") => "reliance reissue",
                    Some("recall") => "reliance recall",
                    _ => "reliance check",
                };
            }
            "guidance" => return "guidance completion",
            "decide" => return "decide card",
            "mirror" => return "mirror",
            "check" => return "check tables",
            "track-record" => return "track-record",
            "recovery" => return "recovery gap",
            "lint" => return "lint",
            "forecast" => {
                let mut next = i + 1;
                while let Some(word) = args.get(next) {
                    match word.as_str() {
                        "--json" | "--trust-host-clock" => next += 1,
                        flag if flag.starts_with("--") => next += 2,
                        "journal" => return "forecast journal",
                        "record" => return "forecast record score",
                        _ => return "forecast assess",
                    }
                }
                return "forecast assess";
            }
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
                        | "--cohort-as-of"
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

fn parse_forecast_journal(
    args: &[String],
    default_root: &Path,
) -> Result<(forecast_journal::Command, OutputFormat, PathBuf), String> {
    use std::collections::BTreeMap;

    use fmine_core::hash::Digest32;
    use forecast_journal::Command;

    fn take(fields: &mut BTreeMap<String, String>, name: &str) -> Result<String, String> {
        fields
            .remove(name)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("missing or empty {name}"))
    }

    fn digest(value: &str) -> Result<Digest32, String> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("digest requires 64 lowercase hexadecimal characters".to_string());
        }
        let mut bytes = [0; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
                .map_err(|error| error.to_string())?;
        }
        Ok(Digest32(bytes))
    }

    let mut fields = BTreeMap::new();
    let mut words = Vec::new();
    let mut index = 0;
    while let Some(argument) = args.get(index) {
        if argument.starts_with("--") {
            let value = if matches!(argument.as_str(), "--json" | "--trust-host-clock") {
                String::new()
            } else {
                index += 1;
                let value = args
                    .get(index)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("missing value for {argument}"))?;
                value.clone()
            };
            if fields.insert(argument.clone(), value).is_some() {
                return Err(format!("duplicate option {argument}"));
            }
        } else {
            words.push(argument.as_str());
        }
        index += 1;
    }
    let ["forecast", "journal", verb] = words.as_slice() else {
        return Err(
            "expected forecast journal init|register|freeze|outcome|resolve|restate|assess|inspect|report"
                .to_string(),
        );
    };
    let format = match fields.remove("--format").as_deref().unwrap_or("json") {
        "json" => OutputFormat::Json,
        "markdown" | "md" => OutputFormat::Markdown,
        _ => return Err("format must be json or markdown".to_string()),
    };
    if fields.remove("--json").is_some() && format != OutputFormat::Json {
        return Err("--json conflicts with --format markdown".to_string());
    }
    let workspace = match fields.remove("--workspace") {
        Some(path) if path.trim().is_empty() => return Err("empty --workspace".to_string()),
        Some(path) => PathBuf::from(path),
        None => default_root.to_path_buf(),
    };
    let journal = PathBuf::from(take(&mut fields, "--journal")?);
    let trust_host_clock = fields.remove("--trust-host-clock").is_some();
    let read_only = matches!(*verb, "inspect" | "report");
    if !read_only && !trust_host_clock {
        return Err(
            "forecast journal mutations require --trust-host-clock for the declared clock policy"
                .to_string(),
        );
    }
    if read_only && trust_host_clock {
        return Err(format!(
            "{verb} reads recorded history and takes no --trust-host-clock"
        ));
    }
    let command = match *verb {
        "init" => {
            let bound = fields
                .remove("--max-clock-skew-ms")
                .map_or(Ok(1_000), |value| {
                    value.parse::<u64>().map_err(|error| error.to_string())
                })?;
            let clock_skew_ns = bound
                .checked_mul(1_000_000)
                .filter(|bound| *bound <= i64::MAX as u64)
                .ok_or("clock-skew bound overflows signed nanoseconds")?;
            Command::Init {
                journal,
                cell_id: take(&mut fields, "--cell")?,
                clock_skew_ns,
            }
        }
        "register" | "freeze" | "outcome" => {
            let head = digest(&take(&mut fields, "--head")?)?;
            let request = PathBuf::from(take(&mut fields, "--request")?);
            match *verb {
                "register" => Command::Register {
                    journal,
                    head,
                    request,
                },
                "freeze" => Command::Freeze {
                    journal,
                    head,
                    request,
                },
                _ => Command::Outcome {
                    journal,
                    head,
                    request,
                },
            }
        }
        "resolve" => Command::Resolve {
            journal,
            head: digest(&take(&mut fields, "--head")?)?,
            cohort: take(&mut fields, "--cohort")?,
            forecast: take(&mut fields, "--forecast")?,
        },
        "restate" => {
            let release = take(&mut fields, "--release")?;
            if !fmine_registry::sources::is_quarter(&release) {
                return Err(
                    "--release requires a canonical YYYYQ1..4 reporting quarter".to_string()
                );
            }
            Command::Restate {
                journal,
                head: digest(&take(&mut fields, "--head")?)?,
                cohort: take(&mut fields, "--cohort")?,
                forecast: take(&mut fields, "--forecast")?,
                release,
            }
        }
        "assess" => {
            let head = digest(&take(&mut fields, "--head")?)?;
            let cohort = take(&mut fields, "--cohort")?;
            let as_of = take(&mut fields, "--as-of")?;
            if fmine_calib::record::parse_utc(&as_of).is_none() {
                return Err("--as-of requires YYYY-MM-DDTHH:MM:SSZ".to_string());
            }
            let selector = fields
                .remove("--selector")
                .unwrap_or_else(|| "first-published".to_string());
            if !matches!(selector.as_str(), "first-published" | "latest-as-of") {
                return Err("--selector must be first-published or latest-as-of".to_string());
            }
            let expected_digest = fields
                .remove("--expected-digest")
                .map(|value| digest(&value))
                .transpose()?;
            Command::Assess {
                journal,
                head,
                cohort,
                as_of,
                selector,
                expected_digest,
            }
        }
        "inspect" => Command::Inspect {
            journal,
            head: fields
                .remove("--head")
                .map(|value| digest(&value))
                .transpose()?,
        },
        "report" => {
            let timestamp = if [
                "--timestamp-head",
                "--timestamp-receipt",
                "--timestamp-roots",
                "--timestamp-intermediates",
            ]
            .iter()
            .any(|option| fields.contains_key(*option))
            {
                Some(forecast_journal::TimestampProof {
                    prefix_head: digest(&take(&mut fields, "--timestamp-head")?)?,
                    receipt: PathBuf::from(take(&mut fields, "--timestamp-receipt")?),
                    trust_roots: PathBuf::from(take(&mut fields, "--timestamp-roots")?),
                    intermediates: if fields.contains_key("--timestamp-intermediates") {
                        Some(PathBuf::from(take(
                            &mut fields,
                            "--timestamp-intermediates",
                        )?))
                    } else {
                        None
                    },
                })
            } else {
                None
            };
            Command::Report {
                journal,
                head: fields
                    .remove("--head")
                    .map(|value| digest(&value))
                    .transpose()?,
                assessment: digest(&take(&mut fields, "--assessment")?)?,
                provenance: fields
                    .remove("--provenance")
                    .map(|path| {
                        if path.trim().is_empty() {
                            Err("empty --provenance".to_string())
                        } else {
                            Ok(PathBuf::from(path))
                        }
                    })
                    .transpose()?,
                timestamp,
            }
        }
        _ => return Err("unknown forecast journal operation".to_string()),
    };
    if let Some(option) = fields.keys().next() {
        return Err(format!(
            "option {option} does not apply to forecast journal {verb}"
        ));
    }
    Ok((command, format, workspace))
}

/// Parses command-line arguments into `(CliCommand, OutputFormat, PathBuf)`.
pub fn parse_args(
    args: &[String],
    default_root: &Path,
) -> Result<(CliCommand, OutputFormat, PathBuf), String> {
    if verb_hint(args) == "assure public" && args.iter().any(|arg| arg == "--output") {
        return reliance::parse_export(args, default_root)
            .map(|(command, format, root)| (CliCommand::AssureExport(command), format, root));
    }
    if verb_hint(args) == "forecast journal" {
        return parse_forecast_journal(args, default_root)
            .map(|(command, format, root)| (CliCommand::ForecastJournal(command), format, root));
    }
    if verb_hint(args) == "forecast record score" {
        return forecast_record::parse(args, default_root)
            .map(|(command, format, root)| (CliCommand::ForecastRecord(command), format, root));
    }
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
    let mut cohort_as_of = None;
    let mut ledger = None;
    let mut cohort = None;
    let mut selector = None;
    let mut capacity_document = None;
    let mut discover = false;
    let mut provenance = None;
    let mut definitions = None;

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
            "--cohort-as-of" => {
                i += 1;
                let value = args.get(i).ok_or("missing value for --cohort-as-of")?;
                if !fmine_registry::sources::is_date(value) {
                    return Err("--cohort-as-of requires YYYY-MM-DD".to_string());
                }
                cohort_as_of = Some(value.clone());
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
            "--definitions" => {
                i += 1;
                if definitions.is_some() {
                    return Err("--definitions may be supplied only once".to_string());
                }
                definitions = Some(PathBuf::from(
                    args.get(i).ok_or("missing value for --definitions")?,
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
        "assure" | "check" | "guidance" | "recovery" | "forecast" | "lint" => 2,
        "claim" | "decide" => 3,
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
        "assure" | "check" | "claim" | "guidance" | "recovery" | "lint" | "track-record"
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
    if positional[0] != "track-record" && cohort_as_of.is_some() {
        return Err("--cohort-as-of requires 'track-record'".to_string());
    }
    if !matches!(
        positional[0].as_str(),
        "guidance" | "forecast" | "track-record" | "lint" | "check"
    ) && as_of.is_some()
    {
        return Err(
            "--as-of requires 'guidance completion', 'track-record', 'forecast assess', 'lint' or PDF 'check tables'"
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
    if definitions.is_some() && positional[0] != "check" {
        return Err("--definitions requires 'check tables'".to_string());
    }

    let cmd = match positional[0].as_str() {
        "decide" => {
            let [_, subcommand, model] = positional.as_slice() else {
                return Err("expected 'fmine decide card <model-file>'".into());
            };
            if subcommand != "card" || model.trim().is_empty() {
                return Err("expected 'fmine decide card <model-file>'".into());
            }
            CliCommand::DecideCard {
                model: PathBuf::from(model),
            }
        }
        "check" => {
            if positional.get(1).map(String::as_str) != Some("tables") {
                return Err("expected 'fmine check tables'".to_string());
            }
            if let Some(cut) = &as_of
                && (definitions.is_none() || !fmine_registry::sources::is_date(cut))
            {
                return Err("check tables --as-of requires --definitions and YYYY-MM-DD".into());
            }
            if synthetic {
                if issuer.is_some() || through.is_some() || definitions.is_some() {
                    return Err(
                        "--synthetic is exclusive with --issuer, --through and --definitions"
                            .into(),
                    );
                }
            } else {
                let name = issuer
                    .as_deref()
                    .ok_or("check tables needs --issuer <issuer> or --synthetic")?;
                if !is_issuer_name(name) || !registry_path(&workspace, name).is_file() {
                    return Err(format!("unknown issuer '{name}'"));
                }
                if definitions.is_some() && through.is_some() {
                    return Err(
                        "--definitions selects PDF identities; --through selects workbook releases"
                            .into(),
                    );
                }
            }
            CliCommand::CheckTables {
                issuer,
                through,
                definitions,
                as_of,
            }
        }
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
            if as_of.is_some() && cohort_as_of.is_some() {
                return Err("track-record selects either --as-of or --cohort-as-of".to_string());
            }
            if through.is_some() && as_of.is_some() {
                return Err("track-record selects either --through or --as-of".to_string());
            }
            if cohort_as_of.is_some() && price_subject.is_some() {
                return Err("track-record --cohort-as-of cannot be combined with --price-subject; price sensitivity is available in retrospective or --as-of mode".to_string());
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
                match cohort_as_of {
                    Some(as_of) => CliCommand::TrackRecordCohortAsOfSynthetic { as_of },
                    None => CliCommand::TrackRecord {
                        issuer: None,
                        site: None,
                        through: None,
                        price_subject: None,
                    },
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
                        price_subject,
                    }
                } else if let Some(as_of) = cohort_as_of {
                    CliCommand::TrackRecordCohortAsOf {
                        issuer,
                        site,
                        through,
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
            if as_of.is_some() && issuer.is_none() {
                return Err("lint --as-of requires registered --issuer sources".to_string());
            }
            if as_of
                .as_deref()
                .is_some_and(|cut| !fmine_registry::sources::is_date(cut))
            {
                return Err("lint --as-of requires YYYY-MM-DD".to_string());
            }
            CliCommand::Lint {
                draft: PathBuf::from(draft),
                issuer,
                through,
                discover,
                as_of,
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

/// DESIGN (BD-347, BD-346): commands with strict selectors keep their own argument grammar. Only
/// presentation and workspace selection are global; evidence, keys and timestamps are never inferred.
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
        (Some("mirror"), _) => CliCommand::Mirror(mirror::parse(&command)?),
        (Some("assure"), Some("issue")) | (Some("reliance"), Some("export")) => {
            CliCommand::Issued(issued::parse(&command, &workspace)?)
        }
        (Some("reliance"), Some("issue" | "reuse" | "reissue" | "recall")) => {
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
        CliCommand::DecideCard { model } => decision::execute(model, workspace_root),
        CliCommand::Mirror(command) => mirror::execute(command, workspace_root),
        CliCommand::RelianceExplicit(command) => {
            reliance_explicit::execute(command, workspace_root)
        }
        CliCommand::Issued(command) => issued::execute(command, workspace_root),
        CliCommand::Reliance(command) => reliance::execute(command, workspace_root),
        CliCommand::AssureExport(command) => {
            let mut envelope = reliance::execute(command, workspace_root);
            envelope.verb = "assure public".to_string();
            envelope
        }
        CliCommand::ForecastJournal(command) => forecast_journal::execute(command, workspace_root),
        CliCommand::ForecastRecord(command) => forecast_record::execute(command, workspace_root),
        CliCommand::CheckTables {
            issuer,
            through,
            definitions,
            as_of,
        } => tables::execute(
            workspace_root,
            issuer.as_deref(),
            through.as_deref(),
            definitions.as_deref(),
            as_of.as_deref(),
        ),
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
            price_subject,
        } => track_record::execute_as_of(
            issuer,
            site.as_deref(),
            as_of,
            price_subject.as_deref(),
            workspace_root,
        ),
        CliCommand::TrackRecordCohortAsOf {
            issuer,
            site,
            through,
            as_of,
        } => track_record_as_of::envelope(track_record_binding::run(
            workspace_root,
            issuer,
            site.as_deref(),
            through.as_deref(),
            as_of,
        )),
        CliCommand::TrackRecordCohortAsOfSynthetic { as_of } => {
            track_record_as_of::envelope(track_record_binding::synthetic_as_of(as_of))
        }
        CliCommand::RecoveryGap { issuer, through } => recovery_envelope(
            fmine_public::recovery::run_public(workspace_root, issuer, through.as_deref()),
        ),
        CliCommand::RecoverySynthetic => recovery_envelope(fmine_public::recovery::run_synthetic()),
        CliCommand::Lint {
            draft,
            issuer,
            through,
            discover,
            as_of,
        } => lint::execute(
            draft,
            issuer.as_deref(),
            through.as_deref(),
            *discover,
            as_of.as_deref(),
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
                        claim_inspection::selected(&report, &profile, id, view, &selector)
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
                "decide card <model-file>",
                "assure public --issuer <issuer> [--through YYYYQn]",
                "assure public --synthetic",
                "assure challenges --issuer <issuer> [--through YYYYQn]",
                "assure challenges --synthetic",
                "check tables --issuer <issuer> [--through YYYYQn]",
                "check tables --issuer <issuer> --definitions <path> [--as-of YYYY-MM-DD]",
                "check tables --synthetic",
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
                "track-record --issuer <issuer> [--site <site>] --as-of YYYY-MM-DD [--price-subject <subject>]",
                "track-record --issuer <issuer> --cohort-as-of YYYY-MM-DD [--site <site>] [--through YYYYQn]",
                "track-record --synthetic --cohort-as-of YYYY-MM-DD",
                "track-record --synthetic [--json]",
                "recovery gap --issuer <issuer> [--through YYYYQn]",
                "recovery gap --synthetic",
                "lint <draft> --issuer <issuer> [--through YYYYQn] [--discover] [--as-of YYYY-MM-DD]",
                "lint <draft> --synthetic [--discover]",
                "forecast assess --ledger <path> --cohort <name> --as-of YYYY-MM-DDTHH:MM:SSZ [--selector first-published|latest-as-of] [--provenance <path>]",
                "forecast journal init --journal <new-path> --cell <cell-id> --trust-host-clock [--max-clock-skew-ms <bound>]",
                "forecast journal register --journal <path> --head <hex> --request <path> --trust-host-clock",
                "forecast journal freeze --journal <path> --head <hex> --request <path> --trust-host-clock",
                "forecast journal outcome --journal <path> --head <hex> --request <path> --trust-host-clock",
                "forecast journal resolve --journal <path> --head <hex> --cohort <name> --forecast <alias> --trust-host-clock",
                "forecast journal restate --journal <path> --head <hex> --cohort <name> --forecast <alias> --release YYYYQn --trust-host-clock",
                "forecast journal assess --journal <path> --head <hex> --cohort <name> --as-of YYYY-MM-DDTHH:MM:SSZ --trust-host-clock [--selector first-published|latest-as-of] [--expected-digest <hex>]",
                "forecast journal inspect --journal <path> [--head <hex>]",
                "forecast record score --record <directory> --as-of YYYY-MM-DDTHH:MM:SSZ --tsa-roots <trusted-pem>",
                "forecast journal report --journal <path> --assessment <digest> [--head <hex>] [--provenance <path>] [--timestamp-head <hex> --timestamp-receipt <path> --timestamp-roots <pem> [--timestamp-intermediates <pem>]]",
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
                "\"cut\":\"Explicit --as-of YYYY-MM-DD; no inferred wall clock; exclusive with --through\",",
                "\"price_sensitivity\":\"Separate paired price sensitivities for first-published and latest registered outcomes; prices and completed quarterly history are bounded by the evidence cut\",",
                "\"population\":\"Earliest registered agreed guidance per subject, metric, fiscal year and ownership basis; revisions remain visible\",",
                "\"outcomes\":\"First-published and latest registered assessments retain missing members\"},",
                "\"cohort_as_of\":{\"mode\":\"publication_cohorts\",\"schema\":\"fmine.guidance.track_record.v1\",",
                "\"cut\":\"Explicit --cohort-as-of YYYY-MM-DD; optional --through bounds releases; exclusive with --as-of and --price-subject\",",
                "\"population\":\"Each guidance publication has a separate metric/year cohort through the shared Claim-checked source binder\",",
                "\"outcomes\":\"Original annual outcomes only; later comparatives remain separate\",",
                "\"missing\":\"Every admitted guidance member remains in its denominator\"},",
                "\"scope\":\"Registered transcription population; no completeness, historical forecast registration, ranking or calibration verdict\"}"
            );
            let mut payload = format!(
                "{{\"binary\":\"fmine\",\"envelope\":\"FMT-ENV-1\",\"envelope_fields\":{ENVELOPE_FIELDS},\"verbs\":[{verbs},{reliance_verbs}],\"issuers\":[{}],\"issuer_registry\":\"registries/sources/<issuer>.toml\",\"exit_codes\":{{\"0\":\"ok; lint verdicts and track-record misses are data\",\"2\":\"usage error (unknown verb, flag, issuer or selection); the envelope still prints\"}},\"claim_inspection\":{{\"lookup\":\"Exact canonical IDs from the synthetic run or hash-verified registered sources; explicit selectors bind the ledger snapshot and claims digest\",\"why\":\"Recorded evidence, interpretation, computed color and Claim admission guards; absent assessments remain absent\",\"trace\":\"Available source anchors and registered provenance; unresolved parents are explicit\"}},\"lint\":{{\"mode\":\"cited equality by default; --discover searches uncited equalities\",\"link\":\"[printed number and unit](fmine:clm_ID)\",\"predicate\":\"One explicit subject, reporting period, ownership basis, supported metric, unit and equality per sentence or line; other prose is unchecked\",\"discovery\":\"All semantic candidates remain visible; multiple matches stay ambiguous and the amount never selects a vintage\",\"scope\":\"Selected registered source snapshot; optional --as-of adds a separate dated public-record assessment with citations and explicit unknown publication dates; no disclosure permission\"}},\"forecast_assessment\":{{\"mode\":\"read_only_replay\",\"input\":\"Ordered forecast, cohort, outcome and assessment TOML rows\",\"default_selector\":\"first-published\",\"provenance\":{{\"schema\":\"fmine.forecast-provenance.v1\",\"tables\":[\"node\",\"anchor\",\"binding\"],\"cut\":\"Each frozen forecast registered_at, not the assessment date\",\"coverage\":\"Every cohort member, including unresolved outcomes\",\"without_bundle\":\"not_checked\",\"generator_replay\":\"not_performed\",\"example\":\"tests/forecasts/provenance_synthetic.toml\"}},\"times_and_citations\":\"Supplied records, not external time proof or independently verified sources\"}},\"reliance\":{reliance_docs},\"track_record\":{track_record_modes},\"refusals\":[{}]}}",
                issuers.join(","),
                refusals.join(",")
            );
            payload.pop();
            payload.push_str(",\"decision\":");
            payload.push_str(decision::ROBOT_DOCS_JSON);
            payload.push_str(concat!(
                ",\"table_checks\":{\"schema\":\"fmine.table-check.v1\",",
                "\"workbooks\":\"Existing assurance identities, source citations and ledger certificate\",",
                "\"definitions\":\"fmine.table-identities.v1; explicit document, member periods and signs, construction per family and convention per document\",",
                "\"pdf_inputs\":\"Every declared document must match its registered bytes; each operand needs agreeing independent reader roles\",",
                "\"findings\":\"Joint exact feasibility, checked conflict certificates, correction sets and implied ranges; no selected correction\",",
                "\"exit\":\"Infeasible identities are findings with exit zero; unreadable or altered bytes and reader disagreements retain typed refusals\"},",
                "\"reliance_explicit\":{\"clock\":\"Caller-supplied --now UTC and --uncertainty-ns; no host clock inference\",",
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

    /// The four INV-002 flags, all false, written by hand from BD-329.S3 (not taken from `FLAGS_JSON`).
    const FLAGS: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                         \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

    fn envelope_for(a: &[&str]) -> Envelope {
        invoke(&args(a), &workspace_root()).0
    }
}
