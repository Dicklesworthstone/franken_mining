//! DESIGN (BD-225): retain an explicit choice and its exact evaluated comparison in the existing
//! locked FJR1 operation journal. A recorded choice is neither an approval nor execution authority.
//! Determinism: Exact for a retained history; admission time comes from the declared host clock.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::forecast_journal::Entry;
use fmine_core::hash::{Digest32, sha256};
use fmine_decide::card;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::forecast::{Journal, MAX_REQUEST_BYTES};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{HostWallClock, WallClock};

use crate::{Envelope, NextAction, OutputFormat, json_string as js, worksheet_input};

const CELL_PREFIX: &str = "fmine.decision/v1:";
const OPERATION_SCHEMA: &str = "fmine.decision-operation.v1";
const REPORT_SCHEMA: &str = "fmine.decision-journal.v1";
pub const NO_CLAIM: &str = "Local record of an explicitly supplied choice and rationale, append-only through this writer. Model inputs remain Assumed, Internal and uncalibrated. The writer retains the evaluated card; replay verifies stored byte integrity, not independent evaluation provenance. This is not a new evaluation or evidence of approval, human identity, sign-off, execution authority or external time proof. Code revision and host-clock accuracy are operator declarations. Only an independently retained final-head pin detects rollback or whole-history rewriting.";
pub const INIT_USAGE: &str = "decide journal init --journal <new-path> --cell <cell-id> --trust-host-clock [--max-clock-skew-ms <bound>]";
pub const RECORD_USAGE: &str = "decide journal record --journal <path> --expected-head <hex> --model <path> --decision <id> --action <id> --reason <text> --code-revision <git-sha> --trust-host-clock";
pub const SHOW_USAGE: &str = "decide journal show --journal <path> [--head <hex>]";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.decision-journal.v1","operations":["init","record","show"],"storage":"Existing FJR1 append-only journal; decision-specific genesis and versioned opaque operations; OS locking, expected-head comparison and sync before success","choice":"One explicit act-now action, including do_nothing, from the evaluated model; any known action may be recorded and the unchanged comparison remains visible","identity":"Unique decision id per journal; another choice needs a new id and rationale; code revision is an explicit 40- or 64-character lowercase Git hash declaration","snapshot":"Exact submitted model bytes and source path, SHA-256, output Claim id and exact evaluated JSON artifact with its own SHA-256; retained artifacts are not re-evaluated on show","bounds":"Every encoded operation must fit the existing 65536-byte limit; oversized models/cards refuse before append","clock":"Mutations require --trust-host-clock; init declares the maximum host-clock skew, default 1000 ms; no external proof or inferred human authorization","show":"Reopens and verifies the complete hash chain and retained content digests without reading the mutable model file; --head pins the exact final head; without it rollback is not checked","scope":"Internal decision support only; no approval, sign-off or execution authorization"}"#;

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
        action: String,
        rationale: String,
        code_revision: String,
    },
    Show {
        journal: PathBuf,
        head: Option<Digest32>,
    },
}

impl Command {
    const fn verb(&self) -> &'static str {
        match self {
            Self::Init { .. } => "decide journal init",
            Self::Record { .. } => "decide journal record",
            Self::Show { .. } => "decide journal show",
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
        return Err("expected decide journal init|record|show".into());
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
    if *verb == "show" && trust_clock {
        return Err("show reads retained history and takes no --trust-host-clock".into());
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
        "record" => Command::Record {
            journal,
            expected_head: digest(&take(&mut fields, "--expected-head")?)?,
            model: PathBuf::from(take(&mut fields, "--model")?),
            decision_id: take(&mut fields, "--decision")?,
            action: take(&mut fields, "--action")?,
            rationale: take(&mut fields, "--reason")?,
            code_revision: take(&mut fields, "--code-revision")?,
        },
        "show" => Command::Show {
            journal,
            head: fields.remove("--head").map(|v| digest(&v)).transpose()?,
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

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn unhex(value: &str) -> Result<String, Refusal> {
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(integrity(
            "retained content must use canonical lowercase hexadecimal",
        ));
    }
    let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
    let bytes: Vec<_> = value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| nibble(p[0]) * 16 + nibble(p[1]))
        .collect();
    String::from_utf8(bytes).map_err(|_| integrity("retained content is not UTF-8"))
}

#[derive(Debug)]
struct Snapshot {
    decision_id: String,
    action: String,
    rationale: String,
    model_path: String,
    model: String,
    card: String,
    claim_id: String,
    code_revision: String,
}

impl Snapshot {
    fn validate(&self) -> Result<(), Refusal> {
        if !clean(&self.decision_id, 256)
            || !clean(&self.action, 256)
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
        Ok(())
    }

    fn operation(&self) -> Result<String, Refusal> {
        self.validate()?;
        let fields = [
            ("schema", OPERATION_SCHEMA.to_owned()),
            ("decision_id", self.decision_id.clone()),
            ("action", self.action.clone()),
            ("rationale", self.rationale.clone()),
            ("model_path", self.model_path.clone()),
            ("model_sha256", sha256(self.model.as_bytes()).to_hex()),
            ("card_sha256", sha256(self.card.as_bytes()).to_hex()),
            ("claim_id", self.claim_id.clone()),
            ("code_revision", self.code_revision.clone()),
            ("model_hex", hex(self.model.as_bytes())),
            ("card_hex", hex(self.card.as_bytes())),
        ];
        let mut out = String::new();
        for (key, value) in fields {
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(out, "{key} = \"{escaped}\"");
        }
        if out.len() > MAX_REQUEST_BYTES {
            return Err(invalid(
                "model and evaluated card exceed the 65536-byte journal operation limit",
            ));
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
        if take(&mut fields, "schema").map_err(integrity)? != OPERATION_SCHEMA {
            return Err(integrity("unknown decision operation schema"));
        }
        let snapshot = Self {
            decision_id: take(&mut fields, "decision_id").map_err(integrity)?,
            action: take(&mut fields, "action").map_err(integrity)?,
            rationale: take(&mut fields, "rationale").map_err(integrity)?,
            model_path: take(&mut fields, "model_path").map_err(integrity)?,
            claim_id: take(&mut fields, "claim_id").map_err(integrity)?,
            code_revision: take(&mut fields, "code_revision").map_err(integrity)?,
            model: unhex(&take(&mut fields, "model_hex").map_err(integrity)?)?,
            card: unhex(&take(&mut fields, "card_hex").map_err(integrity)?)?,
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

    fn json(&self) -> String {
        format!(
            "{{\"decision_id\":{},\"selected_action\":{},\"rationale\":{},\"declared_code_revision\":{},\"claim_id\":{},\"model\":{{\"path\":{},\"sha256\":{},\"bytes\":{},\"content\":{}}},\"artifact\":{{\"media_type\":\"application/json\",\"sha256\":{},\"bytes\":{},\"content\":{}}}}}",
            js(&self.decision_id),
            js(&self.action),
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
        )
    }
}

fn replay(journal: &Journal) -> Result<Vec<Snapshot>, Refusal> {
    if !journal
        .header()
        .cell_id
        .strip_prefix(CELL_PREFIX)
        .is_some_and(|id| clean(id, 256 - CELL_PREFIX.len()))
    {
        return Err(integrity("this is not a decision journal"));
    }
    let mut ids = BTreeSet::new();
    let mut snapshots = Vec::new();
    for record in journal.records().iter().skip(1) {
        let Entry::Operation(operation) = &record.entry else {
            return Err(integrity("unexpected repeated genesis"));
        };
        let snapshot = Snapshot::decode(operation)?;
        if !ids.insert(snapshot.decision_id.clone()) {
            return Err(integrity("a decision id occurs more than once"));
        }
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

fn report(
    journal: &Journal,
    snapshots: &[Snapshot],
    verb: &'static str,
) -> Result<Envelope, Refusal> {
    let mut rows = Vec::new();
    let mut markdown = format!(
        "# Recorded decisions\n\n{NO_CLAIM}\n\nJournal head: `{}`. Decisions: {}.\n",
        journal.head(),
        snapshots.len()
    );
    for (record, snapshot) in journal.records().iter().skip(1).zip(snapshots) {
        let (earliest, latest) = journal.header().time_bounds(record.observed_ns)?;
        rows.push(format!(
            "{{\"sequence\":{},\"record_head\":{},\"observed_ns\":{},\"time_bounds_ns\":[{},{}],\"snapshot\":{}}}",
            record.sequence, js(&record.chain_head()?.to_hex()), js(&record.observed_ns.to_string()),
            js(&earliest.to_string()), js(&latest.to_string()), snapshot.json(),
        ));
        let _ = writeln!(
            markdown,
            "\n## {}\n\nSelected action: {}.\n\nRationale: {}\n\nModel SHA-256: `{}`. Card SHA-256: `{}`.\n\nDeclared code revision: `{}`.\n\nRetained evaluated JSON:\n\n```json\n{}\n```",
            snapshot.decision_id,
            snapshot.action,
            snapshot.rationale,
            sha256(snapshot.model.as_bytes()),
            sha256(snapshot.card.as_bytes()),
            snapshot.code_revision,
            snapshot.card
        );
    }
    let payload = format!(
        "{{\"schema\":\"{REPORT_SCHEMA}\",\"cell_id\":{},\"head\":{},\"final_head_pinned\":{},\"clock_skew_ns\":{},\"n_decisions\":{},\"decisions\":[{}]}}",
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
    envelope.no_claim = Some(NO_CLAIM);
    envelope.rendered_markdown = Some(markdown);
    if !journal.is_pinned() && verb == "decide journal show" {
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

/// Clock injection supports deterministic admission checks; `show` never asks the clock for a time.
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
                let snapshots = replay(&journal)?;
                report(&journal, &snapshots, command.verb())
            }
            Command::Record {
                journal,
                expected_head,
                model,
                decision_id,
                action,
                rationale,
                code_revision,
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
                let inputs = card::model::parse(&text, source.clone())?;
                let evaluated = card::evaluate(&inputs)?;
                if !evaluated
                    .summary()
                    .alternatives
                    .iter()
                    .any(|row| row.action_id == *action)
                {
                    return Err(invalid(
                        "selected action is not an act-now alternative in the evaluated model",
                    ));
                }
                let snapshot = Snapshot {
                    decision_id: decision_id.clone(),
                    action: action.clone(),
                    rationale: rationale.clone(),
                    model_path: source.artifact_path,
                    model: text,
                    card: evaluated.to_json(),
                    claim_id: evaluated.claim().claim_id.to_string(),
                    code_revision: code_revision.clone(),
                };
                let operation = snapshot.operation()?;
                let mut journal = Journal::open_write(&root.join(journal), *expected_head)?;
                let mut snapshots = replay(&journal)?;
                if snapshots.iter().any(|old| old.decision_id == *decision_id) {
                    return Err(Refusal::new(
                        RefusalCode::FrozenRecordMutation507,
                        "decision journal: decision id already exists; retain the original and use a new id",
                    ));
                }
                journal.append(&operation, clock, |_, _| Ok(()))?;
                snapshots.push(snapshot);
                report(&journal, &snapshots, command.verb())
            }
        }
    };
    perform().unwrap_or_else(|refusal| Envelope::refused(command.verb(), refusal, Vec::new()))
}
