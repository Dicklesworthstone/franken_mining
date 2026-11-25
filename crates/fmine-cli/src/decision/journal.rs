//! DESIGN (BD-225): retain an explicit choice and its exact evaluated comparison in the existing
//! locked FJR1 operation journal. A recorded choice is neither an approval nor execution authority.
//! Determinism: Exact for a retained history; admission time comes from the declared host clock.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::forecast_journal::{Entry, MAX_JOURNAL_BYTES};
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::forecast::{
    Journal, MAX_REQUEST_BYTES, decode_hex_field, decode_journal_card, encode_hex_field as hex,
    encode_journal_card,
};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{HostWallClock, WallClock};

use crate::{Envelope, NextAction, OutputFormat, json_string as js, worksheet_input};

mod stage_review;

mod choice;
mod forecasts;
pub use choice::{Choice, PolicySelection};
use forecasts::ForecastLinks;
pub use forecasts::ForecastSelection;

const CELL_PREFIX: &str = "fmine.decision/v1:";
const OPERATION_SCHEMA: &str = "fmine.decision-operation.v1";
const POLICY_OPERATION_SCHEMA: &str = "fmine.decision-operation.v2";
const COMPACT_POLICY_OPERATION_SCHEMA: &str = "fmine.decision-operation.v3";
const LINKED_OPERATION_SCHEMA: &str = "fmine.decision-operation.v4";
const COMPACT_ACTION_SCHEMA: &str = "fmine.decision-operation.v5";
const COMPRESSED_ACTION_SCHEMA: &str = "fmine.decision-operation.v6";
const REPORT_SCHEMA: &str = "fmine.decision-journal.v1";
const POLICY_REPORT_SCHEMA: &str = "fmine.decision-journal.v2";
const LINKED_REPORT_SCHEMA: &str = "fmine.decision-journal.v3";
pub const NO_CLAIM: &str = "Local record of an explicitly supplied choice and rationale, append-only through this writer. Model inputs remain Assumed, Internal and uncalibrated. The writer retains the evaluated card; replay verifies stored byte integrity, not independent evaluation provenance. This is not a new evaluation or evidence of approval, human identity, sign-off, execution authority or external time proof. Code revision and host-clock accuracy are operator declarations. Only an independently retained final-head pin detects rollback or whole-history rewriting.";
pub const INIT_USAGE: &str = "decide journal init --journal <new-path> --cell <cell-id> --trust-host-clock [--max-clock-skew-ms <bound>]";
pub const RECORD_USAGE: &str = "decide journal record --journal <path> --expected-head <hex> --model <path> --decision <id> (--action <id> | --information-option <id> --policy expected-value|downside-optimized) --reason <text> --code-revision <git-sha> [--forecast-journal <path> --forecast-head <hex> --forecast-cohorts <name,...>] --trust-host-clock";
pub const SHOW_USAGE: &str = "decide journal show --journal <path> [--head <hex>]";
pub const ASSESS_USAGE: &str = "decide journal assess --journal <path> --head <hex> --decision <id> --forecast-journal <path> --forecast-head <hex> --assessments <digest,...> [--format json|markdown]";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.decision-journal.v3","operations":["init","record","show","assess"],"storage":"Existing FJR1 append-only journal; decision-specific genesis and versioned opaque operations; OS locking, expected-head comparison and sync before success","choice":"Exactly one --action id or --information-option id with explicit --policy expected-value|downside-optimized; the complete unchanged comparison remains visible","policy":"Expected-value retains the displayed nominal signal policy and posterior ties. Downside-optimized requires a feasible optimizer result and retains its canonical complete mapping, exact co-optimal count and nominal or Wasserstein screen. Choosing an option never implicitly chooses a policy","identity":"Unique decision id per journal; another choice needs a new id and rationale; code revision is an explicit 40- or 64-character lowercase Git hash declaration","snapshot":"Exact model bytes, source path, SHA-256, output Claim id and evaluated JSON artifact with its own SHA-256. Information choices also retain an exact source-bound policy selection artifact and SHA-256. Retained artifacts are not re-evaluated on show","forecast_commitments":"Optional --forecast-journal, --forecast-head and --forecast-cohorts must occur together. One checked current final head binds one through 32 complete frozen cohorts; the shared forecast lock stays held through the decision append. The latest forecast-prefix clock bound must precede the earliest decision time, and the latest decision time must precede every member deadline and plant support start. Recorded member outcomes refuse. Relevance to the choice is declared, not inferred","assess":"Read-only: requires --journal, --head, --decision, --forecast-journal, --forecast-head and one through 64 distinct --assessments digests. Verifies the original prefix in the extended forecast history and retrieves only previously persisted, digest-bound assessments of committed cohorts. Every committed cohort remains visible, including no_assessment_selected; no score is pooled across cohorts and neither journal is changed","compatibility":"Unlinked act-now v1 and information v2/v3 operations retain their existing bytes. Linked decisions use v4 operations and v3 history reports. Assess emits fmine.decision-assessment.v1 without changing the original card, choice or forecasts","bounds":"Every encoded operation must fit the existing 65536-byte limit; oversized models, cards or commitments refuse before append","clock":"Mutations require --trust-host-clock; init declares the maximum host-clock skew, default 1000 ms. Read-only show and assess never sample a clock. Host bounds are operator declarations, not external proof","show":"Reopens and verifies the complete hash chain and retained content digests without reading the mutable model or forecast source; --head pins the exact final head; without it rollback is not checked","scope":"Internal decision support only; forecast scores do not establish decision quality, causality, calibration, approval, sign-off or execution authorization"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Init {
        journal: PathBuf,
        cell_id: String,
        clock_skew_ns: u64,
    },
    Record {
        journal: PathBuf,
        expected_head: Digest32,
        model: PathBuf,
        decision_id: String,
        choice: Choice,
        rationale: String,
        code_revision: String,
        forecasts: Option<ForecastSelection>,
    },
    Show {
        journal: PathBuf,
        head: Option<Digest32>,
    },
    Assess {
        journal: PathBuf,
        head: Digest32,
        decision_id: String,
        forecast_journal: PathBuf,
        forecast_head: Digest32,
        assessments: Vec<Digest32>,
    },
}

impl Command {
    const fn verb(&self) -> &'static str {
        match self {
            Self::Init { .. } => "decide journal init",
            Self::Record { .. } => "decide journal record",
            Self::Show { .. } => "decide journal show",
            Self::Assess { .. } => "decide journal assess",
        }
    }
}

fn take(fields: &mut BTreeMap<String, String>, name: &str) -> Result<String, String> {
    fields
        .remove(name)
        .filter(|value| !value.trim().is_empty() && !value.chars().any(char::is_control))
        .ok_or_else(|| format!("missing or invalid {name}"))
}

fn digest(value: &str) -> Result<Digest32, String> {
    fmine_calib::record::parse_hex32(value)
        .map(Digest32)
        .ok_or_else(|| "digest requires 64 lowercase hexadecimal characters".into())
}

pub(crate) fn parse(
    args: &[String],
    default_root: &Path,
) -> Result<(Command, OutputFormat, PathBuf), String> {
    let (mut fields, words) = crate::journal_arguments(args, &["--json", "--trust-host-clock"])?;
    let ["decide", "journal", verb] = words.as_slice() else {
        return Err("expected decide journal init|record|show|assess".into());
    };
    let format = match fields.remove("--format").as_deref().unwrap_or("json") {
        "json" => OutputFormat::Json,
        "markdown" | "md" => OutputFormat::Markdown,
        _ => return Err("format must be json or markdown".into()),
    };
    if fields.remove("--json").is_some() && format != OutputFormat::Json {
        return Err("--json conflicts with --format markdown".into());
    }
    let workspace = if fields.contains_key("--workspace") {
        PathBuf::from(take(&mut fields, "--workspace")?)
    } else {
        default_root.to_path_buf()
    };
    let journal = PathBuf::from(take(&mut fields, "--journal")?);
    let trust_clock = fields.remove("--trust-host-clock").is_some();
    if matches!(*verb, "init" | "record") && !trust_clock {
        return Err("decision journal mutations require --trust-host-clock".into());
    }
    if matches!(*verb, "show" | "assess") && trust_clock {
        return Err("show and assess read retained history and take no --trust-host-clock".into());
    }
    let command = match *verb {
        "init" => {
            let skew = fields
                .remove("--max-clock-skew-ms")
                .map_or(Ok(1_000), |v| v.parse::<u64>().map_err(|e| e.to_string()))?;
            let clock_skew_ns = skew
                .checked_mul(1_000_000)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or("clock-skew bound overflows signed nanoseconds")?;
            Command::Init {
                journal,
                cell_id: take(&mut fields, "--cell")?,
                clock_skew_ns,
            }
        }
        "record" => {
            let choice = match (
                fields.contains_key("--action"),
                fields.contains_key("--information-option"),
            ) {
                (true, false) => Choice::ActNow(take(&mut fields, "--action")?),
                (false, true) => Choice::InformationOption {
                    option_id: take(&mut fields, "--information-option")?,
                    policy: PolicySelection::parse(&take(&mut fields, "--policy")?)?,
                },
                _ => {
                    return Err(
                        "record requires exactly one --action or --information-option".into(),
                    );
                }
            };
            Command::Record {
                journal,
                expected_head: digest(&take(&mut fields, "--expected-head")?)?,
                model: PathBuf::from(take(&mut fields, "--model")?),
                decision_id: take(&mut fields, "--decision")?,
                choice,
                rationale: take(&mut fields, "--reason")?,
                code_revision: take(&mut fields, "--code-revision")?,
                forecasts: ForecastSelection::parse(&mut fields)?,
            }
        }
        "show" => Command::Show {
            journal,
            head: fields.remove("--head").map(|v| digest(&v)).transpose()?,
        },
        "assess" => Command::Assess {
            journal,
            head: digest(&take(&mut fields, "--head")?)?,
            decision_id: take(&mut fields, "--decision")?,
            forecast_journal: PathBuf::from(take(&mut fields, "--forecast-journal")?),
            forecast_head: digest(&take(&mut fields, "--forecast-head")?)?,
            assessments: forecasts::names(&take(&mut fields, "--assessments")?, 64)?
                .iter()
                .map(|value| digest(value))
                .collect::<Result<Vec<_>, _>>()?,
        },
        _ => return Err("unknown decision journal operation".into()),
    };
    if let Some(name) = fields.keys().next() {
        return Err(format!(
            "option {name} does not apply to decide journal {verb}"
        ));
    }
    Ok((command, format, workspace))
}

fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::UndeclaredUnitBasisOrTz107,
        format!("decision journal: {}", detail.into()),
    )
}

fn integrity(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::DigestMismatch505,
        format!("decision journal: {}", detail.into()),
    )
}

fn clean(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn unhex(value: &str) -> Result<String, Refusal> {
    let bytes = decode_hex_field(value)
        .ok_or_else(|| integrity("retained content must use canonical lowercase hexadecimal"))?;
    String::from_utf8(bytes).map_err(|_| integrity("retained content is not UTF-8"))
}

#[derive(Debug, PartialEq, Eq)]
enum CardEncoding {
    Hex,
    Json,
    // Preserve the stored block so replay's wire check is independent of recompression.
    Lz4(String),
}

#[derive(Debug)]
struct Snapshot {
    decision_id: String,
    choice: Choice,
    policy_snapshot: Option<String>,
    // Replay must retain the original encoding so v1/v2 canonical bytes and heads remain stable.
    encoding: CardEncoding,
    rationale: String,
    model_path: String,
    model: String,
    card: String,
    claim_id: String,
    code_revision: String,
    forecasts: Option<ForecastLinks>,
}

impl Snapshot {
    fn artifact_bytes(&self) -> usize {
        self.model.len() + self.card.len() + self.policy_snapshot.as_ref().map_or(0, String::len)
    }

    fn validate(&self) -> Result<(), Refusal> {
        self.choice.validate()?;
        if !clean(&self.decision_id, 256)
            || !clean(&self.rationale, 4096)
            || !clean(&self.model_path, 4096)
            || !clean(&self.claim_id, 128)
            || !matches!(self.code_revision.len(), 40 | 64)
            || !self
                .code_revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid(
                "bounded ids, rationale, model path and a lowercase Git revision are required",
            ));
        }
        if self.model.is_empty() || self.card.is_empty() {
            return Err(integrity("retained model and card must both be nonempty"));
        }
        match (&self.choice, &self.policy_snapshot) {
            (Choice::ActNow(_), None) => {}
            (Choice::InformationOption { .. }, Some(policy)) if !policy.is_empty() => {}
            _ => {
                return Err(integrity(
                    "retained policy artifact or encoding does not match the choice",
                ));
            }
        }
        if let Some(links) = &self.forecasts {
            links.encode()?;
            if self.encoding != CardEncoding::Json {
                return Err(integrity(
                    "linked decisions require their compact v4 encoding",
                ));
            }
        }
        if matches!(self.encoding, CardEncoding::Lz4(_))
            && !matches!(self.choice, Choice::ActNow(_))
        {
            return Err(integrity("compressed cards require an unlinked action"));
        }
        Ok(())
    }

    fn validate_at(&self, earliest_ns: i64, latest_ns: i64) -> Result<(), Refusal> {
        if let Choice::ActNow(action) = &self.choice {
            crate::capital::staged_journal::validate_at(
                &self.card,
                &self.model,
                action,
                &self.claim_id,
                latest_ns,
            )?;
        }
        if let Some(links) = &self.forecasts {
            links.validate_at(earliest_ns, latest_ns)?;
        }
        Ok(())
    }

    fn operation(&self) -> Result<String, Refusal> {
        self.validate()?;
        let out = self.encode(&self.encoding)?;
        if out.len() <= MAX_REQUEST_BYTES {
            return Ok(out);
        }
        if matches!(self.choice, Choice::ActNow(_)) && self.forecasts.is_none() {
            if self.encoding == CardEncoding::Hex {
                let compact = self.encode(&CardEncoding::Json)?;
                if compact.len() <= MAX_REQUEST_BYTES {
                    return Ok(compact);
                }
            }
            if !matches!(self.encoding, CardEncoding::Lz4(_))
                && let Some(block) = encode_journal_card(&self.card)
            {
                let compact = self.encode(&CardEncoding::Lz4(block))?;
                if compact.len() <= MAX_REQUEST_BYTES {
                    return Ok(compact);
                }
            }
        }
        Err(invalid(
            "model and evaluated card exceed the 65536-byte journal operation limit",
        ))
    }

    fn encode(&self, encoding: &CardEncoding) -> Result<String, Refusal> {
        let mut fields = match &self.choice {
            Choice::ActNow(action) => vec![
                (
                    "schema",
                    match encoding {
                        CardEncoding::Hex => OPERATION_SCHEMA,
                        CardEncoding::Json => COMPACT_ACTION_SCHEMA,
                        CardEncoding::Lz4(_) => COMPRESSED_ACTION_SCHEMA,
                    }
                    .to_owned(),
                ),
                ("decision_id", self.decision_id.clone()),
                ("action", action.clone()),
            ],
            Choice::InformationOption { option_id, policy } => {
                let snapshot = self
                    .policy_snapshot
                    .as_deref()
                    .ok_or_else(|| integrity("missing information-policy snapshot"))?;
                let (schema, policy_field, policy_value) = if *encoding == CardEncoding::Json {
                    (
                        COMPACT_POLICY_OPERATION_SCHEMA,
                        "policy_json",
                        snapshot.to_owned(),
                    )
                } else {
                    (
                        POLICY_OPERATION_SCHEMA,
                        "policy_hex",
                        hex(snapshot.as_bytes()),
                    )
                };
                vec![
                    ("schema", schema.to_owned()),
                    ("decision_id", self.decision_id.clone()),
                    ("information_option", option_id.clone()),
                    ("policy_selection", policy.as_str().to_owned()),
                    ("policy_sha256", sha256(snapshot.as_bytes()).to_hex()),
                    (policy_field, policy_value),
                ]
            }
        };
        fields.extend([
            ("rationale", self.rationale.clone()),
            ("model_path", self.model_path.clone()),
            ("model_sha256", sha256(self.model.as_bytes()).to_hex()),
            ("card_sha256", sha256(self.card.as_bytes()).to_hex()),
            ("claim_id", self.claim_id.clone()),
            ("code_revision", self.code_revision.clone()),
            ("model_hex", hex(self.model.as_bytes())),
            match encoding {
                CardEncoding::Hex => ("card_hex", hex(self.card.as_bytes())),
                CardEncoding::Json => ("card_json", self.card.clone()),
                CardEncoding::Lz4(block) => ("card_lz4_hex", block.clone()),
            },
        ]);
        if let Some(links) = &self.forecasts {
            fields[0].1 = LINKED_OPERATION_SCHEMA.to_owned();
            fields.push(("forecast_commitments_hex", hex(links.encode()?.as_bytes())));
        }
        let mut out = String::new();
        for (key, value) in fields {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(out, "{key} = \"{escaped}\"");
        }
        Ok(out)
    }

    fn decode(operation: &str) -> Result<Self, Refusal> {
        let doc = parse_subset(operation).map_err(|e| integrity(e.to_string()))?;
        if !doc.rows.is_empty() {
            return Err(integrity("unexpected operation table"));
        }
        let mut fields = BTreeMap::new();
        for field in doc.top {
            let TomlValue::Str(value) = field.value else {
                return Err(integrity("every retained operation field must be a string"));
            };
            fields.insert(field.key, value);
        }
        let schema = take(&mut fields, "schema").map_err(integrity)?;
        let encoding = match schema.as_str() {
            COMPACT_POLICY_OPERATION_SCHEMA | LINKED_OPERATION_SCHEMA | COMPACT_ACTION_SCHEMA => {
                CardEncoding::Json
            }
            COMPRESSED_ACTION_SCHEMA => {
                CardEncoding::Lz4(take(&mut fields, "card_lz4_hex").map_err(integrity)?)
            }
            _ => CardEncoding::Hex,
        };
        let (choice, policy_snapshot) = match schema.as_str() {
            OPERATION_SCHEMA
            | COMPACT_ACTION_SCHEMA
            | LINKED_OPERATION_SCHEMA
            | COMPRESSED_ACTION_SCHEMA
                if fields.contains_key("action") =>
            {
                (
                    Choice::ActNow(take(&mut fields, "action").map_err(integrity)?),
                    None,
                )
            }
            POLICY_OPERATION_SCHEMA | COMPACT_POLICY_OPERATION_SCHEMA | LINKED_OPERATION_SCHEMA => {
                let choice = Choice::InformationOption {
                    option_id: take(&mut fields, "information_option").map_err(integrity)?,
                    policy: PolicySelection::parse(
                        &take(&mut fields, "policy_selection").map_err(integrity)?,
                    )
                    .map_err(integrity)?,
                };
                let snapshot = if encoding == CardEncoding::Json {
                    take(&mut fields, "policy_json").map_err(integrity)?
                } else {
                    unhex(&take(&mut fields, "policy_hex").map_err(integrity)?)?
                };
                if digest(&take(&mut fields, "policy_sha256").map_err(integrity)?)
                    .map_err(integrity)?
                    != sha256(snapshot.as_bytes())
                {
                    return Err(integrity(
                        "retained policy_sha256 differs from its exact bytes",
                    ));
                }
                (choice, Some(snapshot))
            }
            _ => return Err(integrity("unknown decision operation schema")),
        };
        let card = match &encoding {
            CardEncoding::Hex => unhex(&take(&mut fields, "card_hex").map_err(integrity)?)?,
            CardEncoding::Json => take(&mut fields, "card_json").map_err(integrity)?,
            CardEncoding::Lz4(block) => decode_journal_card(block)
                .ok_or_else(|| integrity("invalid or oversized compressed journal card"))?,
        };
        let snapshot = Self {
            decision_id: take(&mut fields, "decision_id").map_err(integrity)?,
            choice,
            policy_snapshot,
            encoding,
            rationale: take(&mut fields, "rationale").map_err(integrity)?,
            model_path: take(&mut fields, "model_path").map_err(integrity)?,
            claim_id: take(&mut fields, "claim_id").map_err(integrity)?,
            code_revision: take(&mut fields, "code_revision").map_err(integrity)?,
            forecasts: if schema == LINKED_OPERATION_SCHEMA {
                Some(ForecastLinks::decode(&unhex(
                    &take(&mut fields, "forecast_commitments_hex").map_err(integrity)?,
                )?)?)
            } else {
                None
            },
            model: unhex(&take(&mut fields, "model_hex").map_err(integrity)?)?,
            card,
        };
        for (key, bytes) in [
            ("model_sha256", snapshot.model.as_bytes()),
            ("card_sha256", snapshot.card.as_bytes()),
        ] {
            if digest(&take(&mut fields, key).map_err(integrity)?).map_err(integrity)?
                != sha256(bytes)
            {
                return Err(integrity(format!(
                    "retained {key} differs from its exact bytes"
                )));
            }
        }
        if !fields.is_empty() || snapshot.operation()? != operation {
            return Err(integrity(
                "unknown field or noncanonical decision operation",
            ));
        }
        Ok(snapshot)
    }

    fn json(&self) -> Result<String, Refusal> {
        let forecasts = self.forecasts.as_ref().map_or_else(String::new, |links| {
            format!(",\"forecast_commitments\":{}", links.json())
        });
        let choice = match &self.choice {
            Choice::ActNow(action) => format!("\"selected_action\":{}", js(action)),
            Choice::InformationOption { option_id, policy } => {
                let snapshot = self
                    .policy_snapshot
                    .as_deref()
                    .ok_or_else(|| integrity("missing information-policy snapshot"))?;
                format!(
                    "\"selected_action\":null,\"selected_information_option\":{},\"policy_selection\":{},\"policy_artifact\":{{\"media_type\":\"application/json\",\"sha256\":{},\"bytes\":{},\"content\":{}}}",
                    js(option_id),
                    js(policy.as_str()),
                    js(&sha256(snapshot.as_bytes()).to_hex()),
                    snapshot.len(),
                    js(snapshot),
                )
            }
        };
        Ok(format!(
            "{{\"decision_id\":{},{choice},\"rationale\":{},\"declared_code_revision\":{},\"claim_id\":{},\"model\":{{\"path\":{},\"sha256\":{},\"bytes\":{},\"content\":{}}},\"artifact\":{{\"media_type\":\"application/json\",\"sha256\":{},\"bytes\":{},\"content\":{}}}{forecasts}}}",
            js(&self.decision_id),
            js(&self.rationale),
            js(&self.code_revision),
            js(&self.claim_id),
            js(&self.model_path),
            js(&sha256(self.model.as_bytes()).to_hex()),
            self.model.len(),
            js(&self.model),
            js(&sha256(self.card.as_bytes()).to_hex()),
            self.card.len(),
            js(&self.card),
        ))
    }
}

#[derive(Default)]
struct History {
    snapshots: Vec<Snapshot>,
    ids: BTreeMap<String, usize>,
    stage_reviews: BTreeSet<Digest32>,
    artifact_bytes: usize,
}

impl History {
    fn evidence_bytes_after(&self, additional: usize) -> Option<usize> {
        self.artifact_bytes
            .checked_add(additional)
            .filter(|total| *total <= MAX_JOURNAL_BYTES)
    }
}

fn replay(journal: &Journal) -> Result<History, Refusal> {
    if !journal
        .header()
        .cell_id
        .strip_prefix(CELL_PREFIX)
        .is_some_and(|id| clean(id, 256 - CELL_PREFIX.len()))
    {
        return Err(integrity("this is not a decision journal"));
    }
    let mut history = History::default();
    for record in journal.records().iter().skip(1) {
        let Entry::Operation(operation) = &record.entry else {
            return Err(integrity("unexpected repeated genesis"));
        };
        let snapshot = Snapshot::decode(operation)?;
        history.artifact_bytes = history
            .evidence_bytes_after(snapshot.artifact_bytes())
            .ok_or_else(|| {
                integrity("expanded decision history exceeds the 64 MiB evidence limit")
            })?;
        let (earliest, latest) = journal.header().time_bounds(record.observed_ns)?;
        snapshot.validate_at(earliest, latest)?;
        if let Some(observation) = stage_review::retained(&snapshot)? {
            let (expected, claim_id) =
                observation.prepare(journal, &history, &snapshot.choice, &snapshot.model)?;
            if snapshot.card != expected || snapshot.claim_id != claim_id {
                return Err(integrity(
                    "stage review differs from the worksheet and frozen policy",
                ));
            }
            observation.available_at(earliest)?;
            history.stage_reviews.insert(observation.initial_head);
        }
        if history
            .ids
            .insert(snapshot.decision_id.clone(), history.snapshots.len())
            .is_some()
        {
            return Err(integrity("a decision id occurs more than once"));
        }
        history.snapshots.push(snapshot);
    }
    Ok(history)
}

fn report(
    journal: &Journal,
    snapshots: &[Snapshot],
    verb: &'static str,
) -> Result<Envelope, Refusal> {
    let mut rows = Vec::new();
    let linked = snapshots
        .iter()
        .any(|snapshot| snapshot.forecasts.is_some());
    let no_claim = if linked {
        forecasts::NO_CLAIM
    } else {
        NO_CLAIM
    };
    let mut markdown = format!(
        "# Recorded decisions\n\n{no_claim}\n\nJournal head: `{}`. Decisions: {}.\n",
        journal.head(),
        snapshots.len()
    );
    let summary_start = markdown.len();
    let showing_history = verb == "decide journal show";
    let mut stage_reviews = stage_review::ReviewQueue::default();
    for (record, snapshot) in journal.records().iter().skip(1).zip(snapshots) {
        let (earliest, latest) = journal.header().time_bounds(record.observed_ns)?;
        let record_head = record.chain_head()?;
        if showing_history {
            stage_reviews.add(record_head, snapshot)?;
        }
        rows.push(format!(
            "{{\"sequence\":{},\"record_head\":{},\"observed_ns\":{},\"time_bounds_ns\":[{},{}],\"snapshot\":{}}}",
            record.sequence, js(&record_head.to_hex()), js(&record.observed_ns.to_string()),
            js(&earliest.to_string()), js(&latest.to_string()), snapshot.json()?,
        ));
        let _ = writeln!(
            markdown,
            "\n## {}\n\n{}\n\nRationale: {}\n\nModel SHA-256: `{}`. Card SHA-256: `{}`.\n\nDeclared code revision: `{}`.\n\nRetained evaluated JSON:\n\n```json\n{}\n```",
            snapshot.decision_id,
            snapshot.choice.description(),
            snapshot.rationale,
            sha256(snapshot.model.as_bytes()),
            sha256(snapshot.card.as_bytes()),
            snapshot.code_revision,
            snapshot.card
        );
        if let Some(policy) = &snapshot.policy_snapshot {
            let _ = writeln!(
                markdown,
                "\nRetained selected policy (SHA-256 `{}`):\n\n```json\n{policy}\n```",
                sha256(policy.as_bytes())
            );
        }
        if let Some(links) = &snapshot.forecasts {
            let _ = writeln!(
                markdown,
                "\nProspectively committed forecast cohorts:\n\n```json\n{}\n```",
                links.json()
            );
        }
    }
    let mut summary = String::new();
    stage_reviews.render(&mut summary, journal.is_pinned());
    markdown.insert_str(summary_start, &summary);
    let schema = if linked {
        LINKED_REPORT_SCHEMA
    } else if snapshots.iter().any(|s| s.policy_snapshot.is_some()) {
        POLICY_REPORT_SCHEMA
    } else {
        REPORT_SCHEMA
    };
    let payload = format!(
        "{{\"schema\":\"{schema}\",\"cell_id\":{},\"head\":{},\"final_head_pinned\":{},\"clock_skew_ns\":{},\"n_decisions\":{},\"decisions\":[{}]}}",
        js(&journal.header().cell_id),
        js(&journal.head().to_hex()),
        journal.is_pinned(),
        js(&journal.header().clock_skew_ns.to_string()),
        snapshots.len(),
        rows.join(","),
    );
    let mut envelope = Envelope::ok(
        verb,
        payload,
        vec![NextAction::new(
            "fmine robot-docs",
            "Read decision journal recording and independently retained head requirements",
        )],
    );
    envelope.no_claim = Some(no_claim);
    envelope.rendered_markdown = Some(markdown);
    if showing_history && !journal.is_pinned() {
        envelope.warnings.push(
            "No independently retained final head was supplied; prefix rollback and whole-history rewriting are not checked, and evaluation provenance is unverified.".into(),
        );
    }
    Ok(envelope)
}

#[must_use]
pub fn execute(command: &Command, root: &Path) -> Envelope {
    execute_with_clock(command, root, &HostWallClock)
}

/// Clock injection supports deterministic admission checks; `show` and `assess` never ask for a time.
#[must_use]
pub fn execute_with_clock(command: &Command, root: &Path, clock: &dyn WallClock) -> Envelope {
    let perform = || -> Result<Envelope, Refusal> {
        match command {
            Command::Init {
                journal,
                cell_id,
                clock_skew_ns,
            } => {
                if !clean(cell_id, 256 - CELL_PREFIX.len()) {
                    return Err(invalid(
                        "cell id must be nonempty and bounded without control characters",
                    ));
                }
                let journal = Journal::create(
                    &root.join(journal),
                    format!("{CELL_PREFIX}{cell_id}"),
                    *clock_skew_ns,
                    clock,
                )?;
                report(&journal, &[], command.verb())
            }
            Command::Show { journal, head } => {
                let journal = Journal::open_read(&root.join(journal), *head)?;
                let history = replay(&journal)?;
                report(&journal, &history.snapshots, command.verb())
            }
            Command::Assess {
                journal,
                head,
                decision_id,
                forecast_journal,
                forecast_head,
                assessments,
            } => {
                let journal = Journal::open_read(&root.join(journal), Some(*head))?;
                let history = replay(&journal)?;
                let (record, snapshot) = journal
                    .records()
                    .iter()
                    .skip(1)
                    .zip(&history.snapshots)
                    .find(|(_, snapshot)| snapshot.decision_id == *decision_id)
                    .ok_or_else(|| invalid("decision id is absent from the pinned journal"))?;
                forecasts::review(
                    &journal,
                    snapshot,
                    record.chain_head()?,
                    &root.join(forecast_journal),
                    *forecast_head,
                    assessments,
                )
            }
            Command::Record {
                journal,
                expected_head,
                model,
                decision_id,
                choice,
                rationale,
                code_revision,
                forecasts,
            } => {
                let (text, source) = worksheet_input::read_model(
                    &root.join(model),
                    "decision model",
                    "declared decision model",
                )?;
                if text.len() > MAX_REQUEST_BYTES / 2 {
                    return Err(invalid(
                        "model exceeds the bounded journal operation capacity",
                    ));
                }
                let observation = stage_review::parse_if(&text)?;
                // Regular comparisons need no journal state. A stage review must use the frozen
                // initial record under the same writer lock as the new append.
                let evaluated = if observation.is_none() {
                    let executive = match choice {
                        Choice::ActNow(action) => {
                            crate::capital::executive_snapshot(&text, &source, root, action)?
                        }
                        Choice::InformationOption { .. } => None,
                    };
                    Some(if let Some(executive) = executive {
                        (
                            executive.card,
                            executive.claim_id,
                            None,
                            Some(executive.not_before_ns),
                        )
                    } else if let Choice::ActNow(action) = choice
                        && let Some((card, claim_id)) =
                            crate::capital::program_snapshot(&text, &source, action)?
                    {
                        (card, claim_id, None, None)
                    } else {
                        let (card, claim_id, policy) = choice.snapshot(&text, source.clone())?;
                        (card, claim_id, policy, None)
                    })
                } else {
                    None
                };
                let prepared_forecasts = forecasts
                    .as_ref()
                    .map(|selection| ForecastLinks::admit(selection, root))
                    .transpose()?;
                let mut journal = Journal::open_write(&root.join(journal), *expected_head)?;
                let mut history = replay(&journal)?;
                if history.ids.contains_key(decision_id) {
                    return Err(Refusal::new(
                        RefusalCode::FrozenRecordMutation507,
                        "decision journal: decision id already exists; retain the original and use a new id",
                    ));
                }
                let (card, claim_id, policy_snapshot, not_before_ns) =
                    if let Some(observation) = &observation {
                        let (card, claim_id) =
                            observation.prepare(&journal, &history, choice, &text)?;
                        (card, claim_id, None, None)
                    } else {
                        evaluated.ok_or_else(|| integrity("decision comparison is missing"))?
                    };
                let snapshot = Snapshot {
                    decision_id: decision_id.clone(),
                    choice: choice.clone(),
                    encoding: if policy_snapshot.is_some() || prepared_forecasts.is_some() {
                        CardEncoding::Json
                    } else {
                        CardEncoding::Hex
                    },
                    policy_snapshot,
                    rationale: rationale.clone(),
                    model_path: source.artifact_path,
                    model: text,
                    card,
                    claim_id,
                    code_revision: code_revision.clone(),
                    forecasts: prepared_forecasts.as_ref().map(|(_, links)| links.clone()),
                };
                let operation = snapshot.operation()?;
                if history
                    .evidence_bytes_after(snapshot.artifact_bytes())
                    .is_none()
                {
                    return Err(invalid(
                        "expanded decision history exceeds the 64 MiB evidence limit",
                    ));
                }
                journal.append(&operation, clock, |record, header| {
                    let (earliest, latest) = header.time_bounds(record.observed_ns)?;
                    if not_before_ns.is_some_and(|bound| i128::from(earliest) < bound) {
                        return Err(invalid(
                            "executive worksheet date is after the earliest possible recording time (UTC)",
                        ));
                    }
                    if let Some(observation) = &observation {
                        observation.available_at(earliest)?;
                    }
                    snapshot.validate_at(earliest, latest)
                })?;
                drop(prepared_forecasts);
                history.snapshots.push(snapshot);
                report(&journal, &history.snapshots, command.verb())
            }
        }
    };
    perform().unwrap_or_else(|refusal| Envelope::refused(command.verb(), refusal, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::{History, MAX_JOURNAL_BYTES};

    #[test]
    fn decision_history_applies_limit_to_expanded_artifact_bytes() {
        let history = History {
            artifact_bytes: MAX_JOURNAL_BYTES - 7,
            ..History::default()
        };
        assert_eq!(history.evidence_bytes_after(7), Some(MAX_JOURNAL_BYTES));
        assert_eq!(history.evidence_bytes_after(8), None);
        assert_eq!(history.evidence_bytes_after(usize::MAX), None);
    }
}
