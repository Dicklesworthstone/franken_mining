//! Verified comparative revisions through the public durable-journal interface.
//! Every workbook, issuer, publication date and clock here is invented test evidence.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::Entry;
use fmine_cli::Envelope;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{ClockError, WallClock};

static NEXT: AtomicU64 = AtomicU64::new(0);
const ORIGINAL: &str = "2031-02-16T00:00:00Z";
const REVISION: &str = "2032-02-16T00:00:00Z";
const SECOND: &str = "2033-02-16T00:00:00Z";
const EARLY: &str = "2031-03-01T00:00:00Z";
const LATE: &str = "2032-03-01T00:00:00Z";
const FINAL: &str = "2033-03-01T00:00:00Z";

struct Clock(i64);
impl WallClock for Clock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}
struct NeverClock;
impl WallClock for NeverClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("historical retrieval must not sample a new clock")
    }
}
fn ns(text: &str) -> i64 {
    parse_utc(text)
        .unwrap()
        .checked_mul(1_000_000_000)
        .unwrap()
}
fn string_field<'a>(text: &'a str, field: &str) -> &'a str {
    text.split_once(&format!("\"{field}\":\""))
        .unwrap_or_else(|| panic!("missing {field}: {text}"))
        .1
        .split('"')
        .next()
        .unwrap()
}
fn digest_field(text: &str, field: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(text, field)).unwrap())
}
fn report(text: &str) -> &str {
    text.rsplit_once(",\"report\":")
        .unwrap()
        .1
        .strip_suffix('}')
        .unwrap()
}
fn assessment(text: &str) -> &str {
    text.split_once("\"assessment\":")
        .unwrap()
        .1
        .split_once(",\"checkpoints\":")
        .unwrap()
        .0
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    sources: String,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-restatement-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("corpus")).unwrap();
        fs::create_dir_all(root.join("registries/sources")).unwrap();
        let mut f = Self {
            root,
            head: Digest32::default(),
            sources: String::new(),
        };
        f.release("2030Q4", "2031-02-15", "400.4");
        f.ok(
            Command::Init {
                journal: "journal".into(),
                cell_id: "invented-revisions".into(),
                clock_skew_ns: 0,
            },
            "2030-01-01T00:00:00Z",
        );
        let request = f.request("register.toml", "[[forecast]]\nid = \"one\"\nmodel_key = \"invented\"\noutcome_due = \"2030-12-31T23:59:59Z\"\nkind = \"interval\"\nlo = \"350\"\nhi = \"450\"\nnominal = \"0.8\"\n");
        f.ok(
            Command::Register {
                journal: "journal".into(),
                head: f.head,
                request,
            },
            "2030-01-02T00:00:00Z",
        );
        let request = f.request("freeze.toml", "[[cohort]]\nname = \"site\"\nmembers = \"one\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"example\"\npublic_issuer = \"example\"\npublic_metric = \"production_koz\"\npublic_period = \"2030FY\"\npublic_ownership = \"100%\"\npublic_members = \"one=Site A\"\npublic_vintages = \"registered_comparatives\"\n");
        f.ok(
            Command::Freeze {
                journal: "journal".into(),
                head: f.head,
                request,
            },
            "2030-01-03T00:00:00Z",
        );
        f
    }
    fn release(&mut self, release: &str, publication: &str, raw: &str) {
        let text = |s: &str| MinimalCell::Text(s.into());
        let number = |s: &str| MinimalCell::Styled {
            raw: s.into(),
            xf: 0,
        };
        let sheet = |name: &str, months: &str, value: &str| MinimalSheet {
            name: name.into(),
            hidden: false,
            cells: vec![
                (
                    "A3".into(),
                    text(&format!("For the {months} months ended December 31,")),
                ),
                ("B3".into(), number("2030")),
                ("A5".into(), text("Site A (60%)")),
                ("B4".into(), text("Gold produced (000s oz 100%)")),
                ("B5".into(), number(value)),
            ],
        };
        let bytes = build_minimal_xlsx_styled(
            &[sheet("YTD", "twelve", raw), sheet("QTD", "three", "100.4")],
            Some(&MinimalStyles {
                num_fmts: vec![],
                xfs: vec![1],
            }),
        );
        let file = format!("{release}.xlsx");
        fs::write(self.root.join("corpus").join(&file), &bytes).unwrap();
        self.sources.push_str(&format!(
            "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\npublished_at = \"{publication}\"\npublished_at_basis = \"FACT: invented fixture\"\n",
            bytes.len(), sha256(&bytes).to_hex()
        ));
        fs::write(self.root.join("registries/sources/example.toml"), format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"example\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2033-03-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n[[subject]]\nname = \"Site A\"\n{}", self.sources
        )).unwrap();
    }
    fn request(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }
    fn ok(&mut self, command: Command, at: &str) -> Envelope {
        let result = execute_with_clock(&command, &self.root, &Clock(ns(at)));
        assert_eq!(result.exit_code, 0, "{result:?}");
        self.head = digest_field(&result.payload_json, "head");
        result
    }
    fn original(&mut self) {
        self.ok(
            Command::Resolve {
                journal: "journal".into(),
                head: self.head,
                cohort: "site".into(),
                forecast: "one".into(),
            },
            ORIGINAL,
        );
    }
    fn revision(&self, vintage: &str) -> Command {
        Command::Restate {
            journal: "journal".into(),
            head: self.head,
            cohort: "site".into(),
            forecast: "one".into(),
            release: vintage.into(),
        }
    }
    fn outcome_request(&self, text: &str) -> Command {
        Command::Outcome {
            journal: "journal".into(),
            head: self.head,
            request: self.request("revision.toml", text),
        }
    }
    fn assess(&mut self, cut: &str, selector: &str, now: &str) -> Envelope {
        self.ok(
            Command::Assess {
                journal: "journal".into(),
                head: self.head,
                cohort: "site".into(),
                as_of: cut.into(),
                selector: selector.into(),
                expected_digest: None,
            },
            now,
        )
    }
    fn retrieve(&self, digest: Digest32) -> Envelope {
        let out = execute_with_clock(
            &Command::Report {
                journal: "journal".into(),
                head: Some(self.head),
                assessment: digest,
                provenance: None,
                timestamp: None,
            },
            &self.root,
            &NeverClock,
        );
        assert_eq!(out.exit_code, 0, "{out:?}");
        out
    }
    fn refuse(&self, command: Command, at: &str, expected: &str) {
        let before = fs::read(self.root.join("journal")).unwrap();
        let out = execute_with_clock(&command, &self.root, &Clock(ns(at)));
        assert_ne!(out.exit_code, 0, "{out:?}");
        assert_eq!(out.refusal.as_ref().unwrap().code, expected, "{out:?}");
        assert_eq!(fs::read(self.root.join("journal")).unwrap(), before);
        assert!(Journal::open_read(&self.root.join("journal"), Some(self.head)).is_ok());
    }
}

#[test]
fn verified_revisions_preserve_first_scores_and_historical_report_bytes_across_reopen() {
    let mut f = Fixture::new();
    f.original();
    let early = f.assess(EARLY, "first-published", "2031-03-01T00:00:01Z");
    let early_report = report(&early.payload_json).to_string();
    let early_digest = digest_field(assessment(&early.payload_json), "digest");
    f.release("2031Q4", "2032-02-15", "470.4");
    let revised = f.ok(f.revision("2031Q4"), "2032-02-16T00:00:01Z");
    let before_recording = f.assess(REVISION, "latest-as-of", "2032-02-16T00:00:02Z");
    assert!(assessment(&before_recording.payload_json).contains("\"value\":400,"));
    assert!(assessment(&before_recording.payload_json).contains("\"kind\":\"published\""));
    assert!(revised.payload_json.contains("\"n_outcomes\":2"));
    assert!(revised.payload_json.contains("\"numerator\":\"470\""));
    let first = f.assess(LATE, "first-published", "2032-03-01T00:00:01Z");
    let latest = f.assess(LATE, "latest-as-of", "2032-03-01T00:00:02Z");
    let first_view = assessment(&first.payload_json);
    let latest_view = assessment(&latest.payload_json);
    assert!(first_view.contains("\"value\":400,"), "{first_view}");
    assert!(first_view.contains("\"kind\":\"published\""));
    assert!(latest_view.contains("\"value\":470,"), "{latest_view}");
    assert!(latest_view.contains("\"kind\":\"restated\""));
    assert!(latest_view.contains("\"supersedes\":1"));
    assert!(latest_view.contains("\"vintage\":\"2031Q4\""));
    assert_ne!(
        digest_field(first_view, "digest"),
        digest_field(latest_view, "digest")
    );
    assert_eq!(report(&f.retrieve(early_digest).payload_json), early_report);
    let latest_digest = digest_field(latest_view, "digest");
    let latest_report = report(&latest.payload_json).to_string();
    f.release("2032Q4", "2033-02-15", "0");
    f.ok(f.revision("2032Q4"), SECOND);
    let zero = f.assess(FINAL, "latest-as-of", "2033-03-01T00:00:01Z");
    assert!(assessment(&zero.payload_json).contains("\"value\":0,"));
    assert!(assessment(&zero.payload_json).contains("\"supersedes\":2"));
    fs::rename(f.root.join("corpus"), f.root.join("offline-corpus")).unwrap();
    fs::rename(f.root.join("registries"), f.root.join("offline-registries")).unwrap();
    let before = fs::read(f.root.join("journal")).unwrap();
    assert_eq!(report(&f.retrieve(early_digest).payload_json), early_report);
    assert_eq!(report(&f.retrieve(latest_digest).payload_json), latest_report);
    assert_eq!(fs::read(f.root.join("journal")).unwrap(), before);
}

#[test]
fn original_is_required_and_stale_or_duplicate_revisions_never_append() {
    let mut f = Fixture::new();
    f.release("2031Q4", "2032-02-15", "470.4");
    f.refuse(f.revision("2031Q4"), REVISION, "FMINE-REF-107");
    f.original();
    f.refuse(f.revision("2030Q4"), REVISION, "FMINE-REF-507");
    f.refuse(
        f.revision("2031Q4"),
        "2032-02-15T23:59:59Z",
        "FMINE-REF-206",
    );
    let stale = f.revision("2031Q4");
    f.ok(f.revision("2031Q4"), REVISION);
    f.refuse(stale, "2032-02-16T00:00:01Z", "FMINE-REF-502");
    f.refuse(
        f.revision("2031Q4"),
        "2032-02-16T00:00:01Z",
        "FMINE-REF-107",
    );
}

#[test]
fn callers_cannot_submit_the_generated_revision_receipt_as_an_outcome() {
    let mut f = Fixture::new();
    f.original();
    f.release("2031Q4", "2032-02-15", "470.4");
    f.ok(f.revision("2031Q4"), REVISION);
    let history = Journal::open_read(&f.root.join("journal"), Some(f.head)).unwrap();
    let receipt = history
        .records()
        .iter()
        .find_map(|record| match &record.entry {
            Entry::Operation(request) if request.contains("[[registered_restatement]]") => Some(
                request
                    .lines()
                    .filter(|line| !line.starts_with("schema ="))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .unwrap();
    drop(history);
    f.refuse(
        f.outcome_request(&receipt),
        "2032-02-16T00:00:01Z",
        "FMINE-REF-107",
    );
}

#[test]
fn later_source_authentication_and_publication_guards_apply_before_append() {
    let mut f = Fixture::new();
    f.original();
    f.release("2031Q4", "2032-02-15", "470.4");
    fs::write(f.root.join("corpus/2031Q4.xlsx"), b"tampered").unwrap();
    f.refuse(f.revision("2031Q4"), REVISION, "FMINE-REF-505");
    let mut early = Fixture::new();
    early.original();
    early.release("2031Q4", "2031-02-15", "470.4");
    early.refuse(early.revision("2031Q4"), REVISION, "FMINE-REF-108");
}

#[test]
fn replay_rejects_a_rehashed_receipt_with_a_different_predecessor() {
    let mut f = Fixture::new();
    f.original();
    f.release("2031Q4", "2032-02-15", "470.4");
    f.ok(f.revision("2031Q4"), REVISION);
    let original = Journal::open_read(&f.root.join("journal"), Some(f.head)).unwrap();
    let header = original.header();
    let mut corrupted = Journal::create(
        &f.root.join("corrupted"),
        &header.cell_id,
        header.clock_skew_ns,
        &Clock(original.records()[0].observed_ns),
    )
    .unwrap();
    let mut changed = false;
    for record in original.records() {
        let Entry::Operation(request) = &record.entry else {
            continue;
        };
        let doc = parse_subset(request).unwrap();
        let row = &doc.rows[0];
        let request = if row.table == "registered_restatement" {
            let TomlValue::Int(sequence) = &row.get("supersedes").unwrap().value else {
                panic!("sequence")
            };
            assert_eq!(*sequence, 1);
            changed = true;
            request.replace("supersedes = 1", "supersedes = 2")
        } else {
            request.clone()
        };
        // Deliberately bypass semantic admission to prove hash integrity alone cannot validate a receipt.
        corrupted
            .append(&request, &Clock(record.observed_ns), |_, _| Ok(()))
            .unwrap();
    }
    assert!(changed);
    drop(corrupted);
    let out = execute_with_clock(
        &Command::Inspect {
            journal: "corrupted".into(),
            head: None,
        },
        &f.root,
        &NeverClock,
    );
    assert_eq!(out.refusal.unwrap().code, "FMINE-REF-107");
}

#[test]
fn negative_production_revision_is_refused_without_losing_the_original() {
    let mut f = Fixture::new();
    f.original();
    f.release("2031Q4", "2032-02-15", "-1");
    f.refuse(f.revision("2031Q4"), REVISION, "FMINE-REF-208");
    let current = f.assess(LATE, "latest-as-of", "2032-03-01T00:00:01Z");
    assert!(assessment(&current.payload_json).contains("\"value\":400,"));
    assert!(assessment(&current.payload_json).contains("\"kind\":\"published\""));
    assert!(current.payload_json.contains("\"n_outcomes\":1"));
}
