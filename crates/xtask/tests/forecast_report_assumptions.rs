//! Invented technical reports exercise the real two-reader admission and durable writer.
//! Source bytes are synthetic controls, not PDF extraction or evidence about an issuer.

#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::{Entry, Record, validate_chain};
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{CliCommand, Envelope, parse_args};
use fmine_core::{Digest32, hash::sha256};
use fmine_io::forecast::Journal;
use fmine_runtime::{ClockError, WallClock};

const RULE: &str = "technical-report-assumptions/v1";
const REGISTER: &str = "2029-06-02T00:00:00Z";
const FREEZE: &str = "2029-06-03T00:00:00Z";
const START: &str = "2030-01-01T00:00:00Z";
const END: &str = "2031-01-01T00:00:00Z";
const RESOLVE: &str = "2031-02-01T00:00:00Z";
const CUT: &str = "2031-02-02T00:00:00Z";
const SECOND: i64 = 1_000_000_000;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);
impl WallClock for FixedClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}
fn ns(value: &str) -> i64 {
    parse_utc(value).expect("fixture UTC") * SECOND
}
fn string_field<'a>(json: &'a str, key: &str) -> &'a str {
    json.split_once(&format!("\"{key}\":\""))
        .unwrap_or_else(|| panic!("missing {key}: {json}"))
        .1
        .split('"')
        .next()
        .expect("scalar string")
}
fn digest(json: &str, key: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(json, key)).expect("canonical digest"))
}
fn assessment(json: &str) -> &str {
    json.split_once("\"assessment\":")
        .expect("assessment")
        .1
        .split_once(",\"checkpoints\":")
        .expect("checkpoint boundary")
        .0
}
fn refused(envelope: &Envelope, code: &str) {
    assert_ne!(envelope.exit_code, 0, "{envelope:?}");
    assert_eq!(
        envelope.refusal.as_ref().expect("typed refusal").code,
        code,
        "{envelope:?}"
    );
}

struct Fixture {
    root: PathBuf,
    path: PathBuf,
    head: Digest32,
    request: usize,
}
impl Fixture {
    fn new(skew: u64) -> Self {
        let root = loop {
            let candidate = std::env::temp_dir().join(format!(
                "fmine-report-assumptions-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "cannot reserve report-assumption fixture directory {}: {error}",
                    candidate.display()
                ),
            }
        };
        for dir in ["corpus", "registries/sources", "registries/transcriptions"] {
            fs::create_dir_all(root.join(dir)).expect("fixture directory");
        }
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"synthetic\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2031-02-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n",
        );
        for (file, kind, date) in [
            ("Technical.pdf", "technical_report", "2029-01-01"),
            ("Point.pdf", "technical_report", "2029-01-01"),
            ("Outcome.pdf", "mda", "2031-01-15"),
        ] {
            let bytes = format!("Invented source control: {file}; not an issuer PDF.");
            fs::write(root.join("corpus").join(file), &bytes).expect("source bytes");
            writeln!(registry, "\n[[document]]\nfile = \"{file}\"\ndir = \"corpus\"\nkind = \"{kind}\"\nurl = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\npublished_at = \"{date}\"\npublished_at_basis = \"FACT: synthetic declared publication\"\npages = 1\nbytes = {}\nsha256 = \"{}\"", bytes.len(), sha256(bytes.as_bytes()).to_hex()).expect("format synthetic document registry");
        }
        registry.push_str("\n[[subject]]\nname = \"Example Mine\"\n");
        fs::write(root.join("registries/sources/synthetic.toml"), registry)
            .expect("write synthetic source registry");
        let mut fixture = Self {
            path: root.join("history.journal"),
            root,
            head: Digest32::default(),
            request: 0,
        };
        fixture.readings("125", "usd_per_oz", "2030FY", false);
        fixture.ok(
            Command::Init {
                journal: fixture.path.clone(),
                cell_id: "synthetic-assumptions".into(),
                clock_skew_ns: skew,
            },
            ns("2029-06-01T00:00:00Z"),
        );
        fixture
    }
    fn readings(&self, outcome: &str, unit: &str, period: &str, disagreement: bool) {
        let mut rows =
            String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
        for (file, token) in [
            ("Technical.pdf", "100 - 120"),
            ("Point.pdf", "110"),
            ("Outcome.pdf", outcome),
        ] {
            let hash = sha256(
                &fs::read(self.root.join("corpus").join(file))
                    .expect("read synthetic document bytes"),
            )
            .to_hex();
            for reader in ["reader-a", "reader-b"] {
                let actual = file == "Outcome.pdf";
                let token = if disagreement && actual && reader == "reader-b" {
                    "126"
                } else {
                    token
                };
                let (unit, period) = if actual {
                    (unit, period)
                } else {
                    ("usd_per_oz", "2030FY")
                };
                writeln!(rows, "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{hash}\"\npage = 1\ntable = \"Invented costs\"\nrow = \"Example Mine\"\ncol = \"Cost\"\ntoken = \"{token}\"\nquantity = \"cost\"\nunit = \"{unit}\"\nbasis = \"100%\"\nperiod = \"{period}\"\nreader = \"{reader}\"").expect("format synthetic reader entry");
            }
        }
        fs::write(
            self.root.join("registries/transcriptions/synthetic.toml"),
            rows,
        )
        .expect("write synthetic transcription registry");
    }
    fn run(&self, command: &Command, at: i64) -> Envelope {
        execute_with_clock(command, &self.root, &FixedClock(at))
    }
    fn ok(&mut self, command: Command, at: i64) -> String {
        let envelope = self.run(&command, at);
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        self.head = digest(&envelope.payload_json, "head");
        envelope.payload_json
    }
    fn write(&mut self, text: &str) -> PathBuf {
        let path = self.root.join(format!("request-{}.toml", self.request));
        self.request += 1;
        fs::write(&path, text).expect("fixture declaration");
        path
    }
    fn input(alias: &str, document: &str) -> String {
        format!(
            "schema = \"fmine.report-assumption.v1\"\n[[assumption]]\nid = \"{alias}\"\nissuer = \"synthetic\"\ndocument = \"{document}\"\nquantity = \"cost\"\nsubject = \"Example Mine\"\nperiod = \"2030FY\"\nunit = \"usd_per_oz\"\nbasis = \"100%\"\nphysical_start = \"{START}\"\nphysical_end = \"{END}\"\noutcome_document = \"Outcome.pdf\"\noutcome_quantity = \"cost\"\nauthor_role = \"analyst-role\"\nlane = \"synthetic\"\n"
        )
    }
    fn register(&mut self, alias: &str, document: &str) -> Command {
        let input = self.write(&Self::input(alias, document));
        Command::RegisterAssumption {
            journal: self.path.clone(),
            head: self.head,
            input,
        }
    }
    fn cohort(members: &str) -> String {
        format!(
            "[[cohort]]\nname = \"assumptions\"\nmembers = \"{members}\"\nresolution_rule = \"{RULE}\"\neligible_sources = \"synthetic\"\nalpha = \"0.2\"\nmin_calibration_n = 35\noutcome_lo = \"0\"\noutcome_hi = \"1000\"\n"
        )
    }
    fn freeze(&mut self, text: &str) -> Command {
        let request = self.write(text);
        Command::Freeze {
            journal: self.path.clone(),
            head: self.head,
            request,
        }
    }
    fn resolve(&self, alias: &str) -> Command {
        Command::ResolveAssumption {
            journal: self.path.clone(),
            head: self.head,
            forecast: alias.into(),
        }
    }
    fn inspect(&self) -> String {
        let envelope = self.run(
            &Command::Inspect {
                journal: self.path.clone(),
                head: Some(self.head),
            },
            0,
        );
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        envelope.payload_json
    }
    fn refuse(&self, command: &Command, at: i64, code: &str) {
        let before = fs::read(&self.path).expect("journal bytes before refusal");
        refused(&self.run(command, at), code);
        assert_eq!(
            fs::read(&self.path).expect("journal bytes after refusal"),
            before,
            "refusal cannot append history"
        );
        assert_eq!(digest(&self.inspect(), "head"), self.head);
    }
    fn registered(&mut self) {
        let command = self.register("range", "Technical.pdf");
        self.ok(command, ns(REGISTER));
        let command = self.freeze(&Self::cohort("range"));
        self.ok(command, ns(FREEZE));
    }
}

#[test]
fn source_bound_points_and_ranges_freeze_resolve_and_replay_without_invented_probability() {
    let mut f = Fixture::new(0);
    for (verb, flag, value, expected) in [
        (
            "register-assumption",
            "--input",
            "assumption.toml",
            Command::RegisterAssumption {
                journal: f.path.clone(),
                head: f.head,
                input: "assumption.toml".into(),
            },
        ),
        (
            "resolve-assumption",
            "--forecast",
            "range",
            f.resolve("range"),
        ),
    ] {
        let args = [
            "forecast",
            "journal",
            verb,
            "--journal",
            f.path.to_str().expect("fixture journal path is UTF-8"),
            "--head",
            &f.head.to_hex(),
            flag,
            value,
            "--trust-host-clock",
        ]
        .map(str::to_string);
        let (routed, _, root) = parse_args(&args, &f.root).expect("assumption CLI route");
        assert_eq!(routed, CliCommand::ForecastJournal(expected));
        assert_eq!(root, f.root);
    }
    for (alias, document) in [("range", "Technical.pdf"), ("point", "Point.pdf")] {
        let command = f.register(alias, document);
        let saved = f.ok(command, ns(REGISTER));
        let payload = if alias == "range" {
            "\"lo\":100,\"hi\":120,\"nominal\":null"
        } else {
            "\"lo\":110,\"hi\":110,\"nominal\":null"
        };
        assert!(saved.contains(payload), "{saved}");
        assert!(saved.contains("\"probability_level\":null"));
        assert!(saved.contains("reader-a,reader-b"));
        assert!(saved.contains(RULE));
    }
    let command = f.freeze(&Fixture::cohort("range, point"));
    f.ok(command, ns(FREEZE));
    f.ok(f.resolve("range"), ns(RESOLVE));
    let result = f.ok(
        Command::Assess {
            journal: f.path.clone(),
            head: f.head,
            cohort: "assumptions".into(),
            as_of: CUT.into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        ns(CUT),
    );
    assert!(
        result.contains("\"n_members\":2,\"n_resolved\":1,\"n_unresolved\":1"),
        "{result}"
    );
    assert!(result.contains("Outcome.pdf"));
    assert!(result.contains("\"printed_token\":\"125\""));
    // Descriptive penalty: width 20 + (2 / alpha 0.2) * miss 5 = 70.
    assert!(
        assessment(&result).contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":70")
    );
    assert!(assessment(&result).contains("\"calibration_flag\":\"descriptive\""));
    let checkpoint = digest(assessment(&result), "digest");
    let report = f.run(
        &Command::Report {
            journal: f.path.clone(),
            head: Some(f.head),
            assessment: checkpoint,
            provenance: None,
            timestamp: None,
        },
        0,
    );
    assert_eq!(report.exit_code, 0, "{report:?}");
    assert_eq!(assessment(&report.payload_json), assessment(&result));
    let retained = f.inspect();
    fs::write(
        f.root.join("corpus/Technical.pdf"),
        "changed after admission",
    )
    .expect("change admitted document to test retained replay");
    fs::write(
        f.root.join("registries/transcriptions/synthetic.toml"),
        "unreadable now",
    )
    .expect("replace current registry with unreadable fixture");
    assert_eq!(
        f.inspect(),
        retained,
        "replay verifies retained admissions, not mutable files"
    );
}

#[test]
fn explicit_multi_year_support_retains_source_period_bounds_and_unstated_probability() {
    let mut f = Fixture::new(0);
    let end = "2032-01-01T00:00:00Z";
    let period = format!("UTC [{START},{end})");
    let readings = f.root.join("registries/transcriptions/synthetic.toml");
    let text = fs::read_to_string(&readings)
        .expect("read multi-year transcription fixture")
        .replace("2030FY", &period);
    fs::write(readings, text).expect("write exact multi-year source period");
    let sources = f.root.join("registries/sources/synthetic.toml");
    let text = fs::read_to_string(&sources)
        .expect("read multi-year publication registry")
        .replace("2031-01-15", "2032-01-15")
        .replace("2031-02-01", "2032-02-01");
    fs::write(sources, text).expect("write later multi-year outcome publication");
    let text = Fixture::input("lom", "Technical.pdf")
        .replace("2030FY", &period)
        .replace(END, end);

    // Keep the source period unchanged while changing only the declared physical end.
    let bad = text.replace(
        &format!("physical_end = \"{end}\""),
        "physical_end = \"2032-01-02T00:00:00Z\"",
    );
    let input = f.write(&bad);
    f.refuse(
        &Command::RegisterAssumption {
            journal: f.path.clone(),
            head: f.head,
            input,
        },
        ns(REGISTER),
        "FMINE-REF-107",
    );
    let input = f.write(&text);
    let admitted = f.ok(
        Command::RegisterAssumption {
            journal: f.path.clone(),
            head: f.head,
            input,
        },
        ns(REGISTER),
    );
    assert!(admitted.contains("\"lo\":100,\"hi\":120,\"nominal\":null"));
    assert!(admitted.contains("\"probability_level\":null"));
    assert!(admitted.contains(&format!("\"period\":\"{period}\"")));
    assert!(admitted.contains(&format!("\"outcome_due_ns\":{}", ns(end))));

    let command = f.freeze(&Fixture::cohort("lom"));
    f.ok(command, ns(FREEZE));
    f.ok(f.resolve("lom"), ns("2032-02-01T00:00:00Z"));
    let cut = "2032-02-02T00:00:00Z";
    let scored = f.ok(
        Command::Assess {
            journal: f.path.clone(),
            head: f.head,
            cohort: "assumptions".into(),
            as_of: cut.into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        ns(cut),
    );
    assert!(scored.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0"));
    assert!(scored.contains("\"published_at\":\"2032-01-15\""));
    assert!(
        assessment(&scored).contains("\"interval_score\":{\"n_scored\":1,\"observed_mean\":70")
    );
    assert!(assessment(&scored).contains("\"calibration_flag\":\"descriptive\""));
}

#[test]
fn source_failures_and_mismatched_declarations_never_append_a_forecast() {
    for case in 0..8 {
        let mut f = Fixture::new(0);
        let mut input = Fixture::input("range", "Technical.pdf");
        match case {
            0 => fs::write(f.root.join("corpus/Technical.pdf"), "altered")
                .expect("alter assumption source bytes"),
            1 => {
                let path = f.root.join("registries/sources/synthetic.toml");
                let text = fs::read_to_string(&path)
                    .expect("read source location fixture")
                    .replacen("\ndir = \"corpus\"\n", "\ndir = \"absent\"\n", 1);
                fs::write(path, text).expect("point source registry at missing document");
            }
            2 => {
                let path = f.root.join("registries/transcriptions/synthetic.toml");
                let text = fs::read_to_string(&path)
                    .expect("read agreement fixture")
                    .replacen("token = \"100 - 120\"", "token = \"100 - 130\"", 1);
                fs::write(path, text).expect("write conflicting source reading");
            }
            3 => input = input.replace("usd_per_oz", "kt"),
            4 => input = input.replace("2030FY", "2031FY"),
            5 => input = input.replace(END, "2030-12-31T00:00:00Z"),
            6 => input.push_str("nominal = \"0.8\"\n"),
            _ => input = input.replace("Technical.pdf", "Outcome.pdf"),
        }
        let input = f.write(&input);
        let command = Command::RegisterAssumption {
            journal: f.path.clone(),
            head: f.head,
            input,
        };
        f.refuse(
            &command,
            ns(REGISTER),
            match case {
                0 => "FMINE-REF-505",
                1 => "FMINE-REF-506",
                2 => "FMINE-REF-202",
                _ => "FMINE-REF-107",
            },
        );
    }
}

#[test]
fn frozen_rule_issuer_and_supplied_outcome_cannot_bypass_source_resolution() {
    let mut f = Fixture::new(0);
    let command = f.register("range", "Technical.pdf");
    f.ok(command, ns(REGISTER));
    for wrong in [
        Fixture::cohort("range").replace(RULE, "supplied"),
        Fixture::cohort("range").replace(
            "eligible_sources = \"synthetic\"",
            "eligible_sources = \"other\"",
        ),
    ] {
        let command = f.freeze(&wrong);
        f.refuse(&command, ns(FREEZE), "FMINE-REF-107");
    }
    let command = f.freeze(&Fixture::cohort("range"));
    f.ok(command, ns(FREEZE));
    let text = format!(
        "[[outcome]]\nforecast = \"range\"\nkind = \"published\"\navailable_at = \"2031-01-16T00:00:00Z\"\nvalue = \"999\"\nsource = \"synthetic\"\nvintage = \"Outcome.pdf\"\nlocator = \"invented\"\nsha256 = \"{}\"\n",
        "a".repeat(64)
    );
    let request = f.write(&text);
    f.refuse(
        &Command::Outcome {
            journal: f.path.clone(),
            head: f.head,
            request,
        },
        ns(RESOLVE),
        "FMINE-REF-107",
    );
    f.ok(f.resolve("range"), ns(RESOLVE));
    f.refuse(&f.resolve("range"), ns(CUT), "FMINE-REF-507");
}

#[test]
fn outcome_bytes_readers_unit_period_and_finite_point_are_checked_against_the_frozen_target() {
    for case in 0..6 {
        let mut f = Fixture::new(0);
        f.registered();
        match case {
            0 => fs::write(f.root.join("corpus/Outcome.pdf"), "altered outcome")
                .expect("alter outcome source bytes"),
            1 => f.readings("125", "kt", "2030FY", false),
            2 => f.readings("125", "usd_per_oz", "2031FY", false),
            3 => f.readings("125", "usd_per_oz", "2030FY", true),
            4 => f.readings("125 - 130", "usd_per_oz", "2030FY", false),
            _ => {
                let path = f.root.join("registries/sources/synthetic.toml");
                let text = fs::read_to_string(&path)
                    .expect("read outcome publication fixture")
                    .replace("2031-01-15", "2030-12-30");
                fs::write(path, text).expect("write premature outcome publication");
            }
        }
        f.refuse(
            &f.resolve("range"),
            ns(RESOLVE),
            match case {
                0 => "FMINE-REF-505",
                3 => "FMINE-REF-202",
                5 => "FMINE-REF-206",
                _ => "FMINE-REF-107",
            },
        );
    }
}

#[test]
fn both_clock_bounds_protect_registration_freeze_and_resolution() {
    let mut f = Fixture::new(2 * SECOND as u64);
    let command = f.register("range", "Technical.pdf");
    f.refuse(&command, ns(START) - SECOND, "FMINE-REF-206");
    f.ok(command, ns(REGISTER));
    let command = f.freeze(&Fixture::cohort("range"));
    f.refuse(&command, ns(START) - SECOND, "FMINE-REF-107");
    f.ok(command, ns(FREEZE));
    f.refuse(
        &f.resolve("range"),
        ns("2031-01-16T00:00:01Z"),
        "FMINE-REF-206",
    );
    f.ok(f.resolve("range"), ns("2031-01-16T00:00:02Z"));
}

fn write_rechained(path: &Path, records: &mut [Record]) -> Digest32 {
    let mut previous = Digest32::default();
    for record in records.iter_mut() {
        record.previous = previous;
        previous = record
            .chain_head()
            .expect("recompute negative fixture head");
    }
    validate_chain(records).expect("negative fixture has a valid byte chain");
    let bytes: Vec<_> = records
        .iter()
        .flat_map(|r| r.to_line().expect("encode negative fixture record"))
        .collect();
    fs::write(path, bytes).expect("write rechained negative fixture");
    previous
}

#[test]
fn replay_requires_v15_for_both_admissions_even_after_the_byte_chain_is_recomputed() {
    let mut f = Fixture::new(0);
    f.registered();
    f.ok(f.resolve("range"), ns(RESOLVE));
    let original = Journal::open_read(&f.path, Some(f.head))
        .expect("reopen admitted source journal")
        .records()
        .to_vec();
    for table in ["assumption_forecast", "assumption_outcome"] {
        for schema in ["schema = \"fmine.forecast-operation.v14\"\n", ""] {
            let mut records = original.clone();
            let text = records
                .iter_mut()
                .find_map(|r| match &mut r.entry {
                    Entry::Operation(text) if text.contains(&format!("[[{table}]]")) => Some(text),
                    _ => None,
                })
                .expect("stored source operation");
            assert!(text.starts_with("schema = \"fmine.forecast-operation.v15\"\n"));
            *text = text.replacen("schema = \"fmine.forecast-operation.v15\"\n", schema, 1);
            let path = f
                .root
                .join(format!("downgraded-{table}-{}.journal", schema.len()));
            let pin = write_rechained(&path, &mut records);
            refused(
                &f.run(
                    &Command::Inspect {
                        journal: path,
                        head: Some(pin),
                    },
                    0,
                ),
                "FMINE-REF-107",
            );
        }
    }
}
