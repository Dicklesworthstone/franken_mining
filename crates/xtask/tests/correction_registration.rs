//! Prospective correction series through the durable writer and authenticated synthetic public cells.
//! Every issuer, workbook, forecast and clock is invented; no real-filing or external-time claim.
//! Determinism: DeterministicPerIsa
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::Entry;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{CliCommand, Envelope, parse_args};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal as DurableJournal;
use fmine_io::omf::json::parse;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_runtime::{ClockError, WallClock};

const SECOND: i64 = 1_000_000_000;
const JOURNAL: &str = "correction.journal";
const RAW_MODEL: &str = "synthetic:raw-production-v1";
const CORRECTED_MODEL: &str = "synthetic:corrected-production-v1";
const TARGET_DUE: &str = "2032-12-31T23:59:59Z";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock(i64);

impl WallClock for FixedClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}

struct NeverClock;

impl WallClock for NeverClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("a saved proposal or journal replay must not sample a clock")
    }
}

fn ns(value: &str) -> i64 {
    parse_utc(value).unwrap().checked_mul(SECOND).unwrap()
}

fn string_field<'a>(json: &'a str, key: &str) -> &'a str {
    json.split_once(&format!("\"{key}\":\""))
        .unwrap_or_else(|| panic!("missing {key}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

fn digest_field(json: &str, key: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(json, key)).unwrap())
}

fn number_field(json: &str, key: &str) -> f64 {
    json.split_once(&format!("\"{key}\":"))
        .unwrap_or_else(|| panic!("missing {key}: {json}"))
        .1
        .split([',', '}'])
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

// Extract one balanced object from controlled output, retaining its exact bytes for replay checks.
fn object(json: &str) -> &str {
    assert!(json.starts_with('{'));
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, byte) in json.bytes().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &json[..=offset];
                    }
                }
                _ => {}
            }
        }
    }
    panic!("unterminated controlled JSON object")
}

fn forecast<'a>(json: &'a str, alias: &str) -> &'a str {
    let inventory = json.split_once("\"forecasts\":[").unwrap().1;
    let start = inventory
        .find(&format!("{{\"alias\":\"{alias}\","))
        .unwrap_or_else(|| panic!("missing forecast {alias}: {inventory}"));
    object(&inventory[start..])
}

fn assessment(json: &str) -> &str {
    object(json.split_once("\"assessment\":").unwrap().1)
}

fn correction(json: &str) -> &str {
    object(json.split_once("\"correction\":").unwrap().1)
}

fn declared_forecast(alias: &str, mean: u32, due: &str) -> String {
    format!(
        "[[forecast]]\nid = \"{alias}\"\nmodel_key = \"{RAW_MODEL}\"\noutcome_due = \"{due}\"\nkind = \"gaussian\"\nmean = \"{mean}\"\nstd_dev = \"1\"\nfamily = \"synthetic-production\"\nauthor_role = \"raw-model-role\"\ndomain = \"one-domain\"\nhorizon = \"year\"\nsite = \"Site A\"\nquantity = \"production_koz\"\nunit = \"koz\"\n"
    )
}

fn supplied_outcome(alias: &str, value: u32, year: u16) -> String {
    format!(
        "[[outcome]]\nforecast = \"{alias}\"\nkind = \"published\"\navailable_at = \"{}-02-01T00:00:00Z\"\nvalue = \"{value}\"\nsource = \"synthetic:issuer\"\nvintage = \"synthetic-{year}\"\nlocator = \"Invented!{alias}\"\nsha256 = \"{}\"\n",
        year + 1,
        "11".repeat(32),
    )
}

fn sheet(name: &str, months: &str, year: u16, production: u32) -> MinimalSheet {
    let text = |value: &str| MinimalCell::Text(value.into());
    let number = |value: u32| MinimalCell::Styled {
        raw: value.to_string(),
        xf: 0,
    };
    MinimalSheet {
        name: name.into(),
        hidden: false,
        cells: vec![
            (
                "A3".into(),
                text(&format!("For the {months} months ended December 31,")),
            ),
            ("B3".into(), number(u32::from(year))),
            ("A5".into(), text("Site A (60%)")),
            ("B4".into(), text("Gold produced (000s oz 100%)")),
            ("B5".into(), number(production)),
            ("C4".into(), text("Gold produced (000s oz attributable)")),
            ("C5".into(), number(production * 3 / 5)),
        ],
    }
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    parents: Vec<Digest32>,
    saved_assessments: Vec<String>,
    source_hashes: Vec<Digest32>,
    public: bool,
    skew: u64,
    next_request: usize,
}

impl Fixture {
    fn empty(public: bool, skew: u64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-correction-registration-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut fixture = Self {
            root,
            head: Digest32::default(),
            parents: Vec::new(),
            saved_assessments: Vec::new(),
            source_hashes: Vec::new(),
            public,
            skew,
            next_request: 0,
        };
        if public {
            fixture.write_public_sources();
        }
        fixture.ok(
            Command::Init {
                journal: JOURNAL.into(),
                cell_id: "synthetic-correction-cell".into(),
                clock_skew_ns: skew,
            },
            ns("2030-01-01T00:00:00Z"),
        );
        fixture
    }

    fn series(public: bool, skew: u64, freeze_target: bool) -> Self {
        let mut fixture = Self::empty(public, skew);
        for (year, mean, actual) in [(2030, 10, 20), (2031, 20, 40)] {
            let alias = format!("train-{year}");
            let cohort = format!("training-{year}");
            let start = ns(&format!("{year}-02-05T00:00:00Z"));
            fixture.append(
                &declared_forecast(&alias, mean, &format!("{year}-12-31T23:59:59Z")),
                start,
            );
            fixture.append(&fixture.cohort(&cohort, &alias, year), start + SECOND);
            fixture.resolve(&cohort, &alias, year, actual);
            fixture.save_assessment(&cohort, &format!("{}-02-03T00:00:00Z", year + 1));
        }
        fixture.future(freeze_target);
        fixture
    }

    fn single() -> Self {
        let mut fixture = Self::empty(false, 0);
        let start = ns("2030-02-05T00:00:00Z");
        for (index, alias, mean) in [(0, "train-a", 10), (1, "train-b", 20)] {
            fixture.append(
                &declared_forecast(alias, mean, "2030-12-31T23:59:59Z"),
                start + index * SECOND,
            );
        }
        fixture.append(
            &fixture.cohort("training-single", "train-a,train-b", 2030),
            start + 2 * SECOND,
        );
        fixture.append(
            &supplied_outcome("train-a", 20, 2030),
            ns("2031-02-02T00:00:00Z"),
        );
        fixture.append(
            &supplied_outcome("train-b", 40, 2030),
            ns("2031-02-02T00:00:01Z"),
        );
        fixture.save_assessment("training-single", "2031-02-03T00:00:00Z");
        fixture.future(true);
        fixture
    }

    fn future(&mut self, freeze: bool) {
        self.append(
            &declared_forecast("raw", 50, TARGET_DUE),
            ns("2032-02-04T00:00:00Z"),
        );
        if freeze {
            self.append(
                &self.cohort("future-raw", "raw", 2032),
                ns("2032-02-05T00:00:00Z"),
            );
        }
    }

    fn write_public_sources(&mut self) {
        for directory in ["corpus", "registries/sources"] {
            fs::create_dir_all(self.root.join(directory)).unwrap();
        }
        let mut registry = String::from(
            "schema = \"fmine.sources.v1\"\nissuer = \"example\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2033-03-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\n[[subject]]\nname = \"Site A\"\n",
        );
        for (year, actual) in [(2030, 20), (2031, 40), (2032, 100)] {
            let bytes = build_minimal_xlsx_styled(
                &[
                    sheet("YTD", "twelve", year, actual),
                    sheet("QTD", "three", year, actual / 4),
                ],
                Some(&MinimalStyles {
                    num_fmts: vec![],
                    xfs: vec![1],
                }),
            );
            let digest = sha256(&bytes);
            self.source_hashes.push(digest);
            let file = format!("{year}Q4.xlsx");
            fs::write(self.root.join("corpus").join(&file), &bytes).unwrap();
            registry.push_str(&format!(
                "\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{year}Q4\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{digest}\"\npublished_at = \"{}-02-01\"\npublished_at_basis = \"FACT: invented correction source fixture\"\n",
                bytes.len(),
                year + 1,
            ));
        }
        fs::write(self.root.join("registries/sources/example.toml"), registry).unwrap();
    }

    fn cohort(&self, name: &str, members: &str, year: u16) -> String {
        let policy = "alpha = \"0.2\"\nmin_calibration_n = 35\nrho_low = \"0\"\nrho_high = \"0.5\"\noutcome_lo = \"0\"\noutcome_hi = \"1000\"\n";
        if self.public {
            format!(
                "[[cohort]]\nname = \"{name}\"\nmembers = \"{members}\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"example\"\npublic_issuer = \"example\"\npublic_metric = \"production_koz\"\npublic_period = \"{year}FY\"\npublic_ownership = \"100%\"\npublic_members = \"{members}=Site A\"\n{policy}"
            )
        } else {
            format!(
                "[[cohort]]\nname = \"{name}\"\nmembers = \"{members}\"\nresolution_rule = \"Synthetic annual output on a fixed basis\"\neligible_sources = \"synthetic:issuer\"\n{policy}"
            )
        }
    }

    fn request(&mut self, text: &str) -> Command {
        let request = self
            .root
            .join(format!("request-{}.toml", self.next_request));
        self.next_request += 1;
        fs::write(&request, text).unwrap();
        let journal = JOURNAL.into();
        let head = self.head;
        match text.lines().next().unwrap() {
            "[[forecast]]" => Command::Register {
                journal,
                head,
                request,
            },
            "[[cohort]]" => Command::Freeze {
                journal,
                head,
                request,
            },
            "[[outcome]]" => Command::Outcome {
                journal,
                head,
                request,
            },
            table => panic!("unexpected test request {table}"),
        }
    }

    fn append(&mut self, text: &str, at: i64) -> Envelope {
        let request = self.request(text);
        self.ok(request, at)
    }

    fn ok(&mut self, command: Command, at: i64) -> Envelope {
        let envelope = execute_with_clock(&command, &self.root, &FixedClock(at));
        assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
        self.head = digest_field(&envelope.payload_json, "head");
        envelope
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(JOURNAL)).unwrap()
    }

    fn refuse(&self, command: &Command, at: i64) -> Envelope {
        let before = self.bytes();
        let envelope = execute_with_clock(command, &self.root, &FixedClock(at));
        assert_ne!(envelope.exit_code, 0, "{}", envelope.to_json());
        assert!(envelope.refusal.is_some(), "typed refusal required");
        assert_eq!(
            self.bytes(),
            before,
            "a refused operation must append no bytes"
        );
        assert!(DurableJournal::open_read(&self.root.join(JOURNAL), Some(self.head)).is_ok());
        envelope
    }

    fn resolve(&mut self, cohort: &str, alias: &str, year: u16, actual: u32) -> Envelope {
        let at = ns(&format!("{}-02-02T00:00:00Z", year + 1));
        self.resolve_at(cohort, alias, year, actual, at)
    }

    fn resolve_at(
        &mut self,
        cohort: &str,
        alias: &str,
        year: u16,
        actual: u32,
        at: i64,
    ) -> Envelope {
        if self.public {
            self.ok(
                Command::Resolve {
                    journal: JOURNAL.into(),
                    head: self.head,
                    cohort: cohort.into(),
                    forecast: alias.into(),
                },
                at,
            )
        } else {
            self.append(&supplied_outcome(alias, actual, year), at)
        }
    }

    fn save_assessment(&mut self, cohort: &str, cut: &str) -> Envelope {
        let saved = self.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: self.head,
                cohort: cohort.into(),
                as_of: cut.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns(cut) + i64::try_from(self.skew).unwrap(),
        );
        let body = assessment(&saved.payload_json);
        self.parents.push(digest_field(body, "digest"));
        self.saved_assessments.push(body.to_string());
        saved
    }

    fn proposal_command(&self) -> Command {
        if self.parents.len() == 1 {
            Command::Correction {
                journal: JOURNAL.into(),
                head: Some(self.head),
                assessment: self.parents[0],
                forecast: "raw".into(),
            }
        } else {
            Command::CorrectionSeries {
                journal: JOURNAL.into(),
                head: Some(self.head),
                assessments: self.parents.clone(),
                forecast: "raw".into(),
            }
        }
    }

    fn proposal(&self) -> Envelope {
        let result = execute_with_clock(&self.proposal_command(), &self.root, &NeverClock);
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        result
    }

    fn register_command(&self, proposal: Digest32) -> Command {
        Command::RegisterCorrection {
            journal: JOURNAL.into(),
            head: self.head,
            id: "corrected".into(),
            forecast: "raw".into(),
            assessments: self.parents.clone(),
            series: self.parents.len() > 1,
            expected_proposal: proposal,
            model_key: CORRECTED_MODEL.into(),
            author_role: "review-role".into(),
        }
    }

    fn inspect(&self) -> Envelope {
        let result = execute_with_clock(
            &Command::Inspect {
                journal: JOURNAL.into(),
                head: Some(self.head),
            },
            &self.root,
            &NeverClock,
        );
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        result
    }
}

#[test]
fn public_series_retains_every_parent_and_source_without_sampling_time_or_appending() {
    let fixture = Fixture::series(true, 0, true);
    let before = fixture.bytes();
    let result = fixture.proposal();
    parse(result.payload_json.as_bytes()).unwrap();
    let report = correction(&result.payload_json);
    assert!(report.contains("\"schema\":\"fmine.forecast-correction-series.v1\""));
    assert!(report.contains("\"digest_encoding\":\"COR2/v1\""));
    assert!(report.contains("\"n_assessments\":2,\"n_training\":2"));
    assert!(report.contains("\"status\":\"unissued_proposal\",\"uncalibrated\":true"));
    assert!((number_field(report, "bias_factor") - 2.0).abs() < 1e-12);
    assert_eq!(number_field(report, "raw_training_hits"), 0.0);
    assert_eq!(number_field(report, "corrected_training_hits"), 2.0);
    for (index, year) in [2030, 2031].into_iter().enumerate() {
        assert!(report.contains(&format!(
            "\"assessment_digest\":\"{}\"",
            fixture.parents[index]
        )));
        assert!(report.contains(&format!("\"name\":\"training-{year}\"")));
        assert!(report.contains(&format!("\"period\":\"{year}FY\"")));
        assert!(report.contains(&format!("\"as_of\":\"{}-02-03T00:00:00Z\"", year + 1)));
        assert!(report.contains(&fixture.source_hashes[index].to_hex()));
        assert!(report.contains(&format!("{year}Q4.xlsx")));
    }
    for required in [
        "\"target_cohorts\":[{\"cohort\":",
        "\"name\":\"future-raw\"",
        "\"period\":\"2032FY\"",
        "\"members\":[{\"forecast\":\"raw\"",
        "\"min_calibration_n\":35",
        "\"correlation_range\":[0,0.5]",
        "\"outcome_bounds\":[0,1000]",
        "\"selected_event\":",
        "\"available_at\":",
        "\"recorded_at\":",
        "\"source_admission\":{",
        "\"raw\":{\"kind\":\"gaussian\",\"mean\":50,\"std_dev\":1}",
    ] {
        assert!(report.contains(required), "missing {required}: {report}");
    }
    let markdown = result.rendered_markdown.as_deref().unwrap();
    for required in [
        "Unissued and uncalibrated",
        "COR2/v1",
        "training-2030",
        "training-2031",
        "future-raw",
        "Complete membership",
        "Retained public source admission",
        "not claim to be a complete historical record",
    ] {
        assert!(
            markdown.contains(required),
            "missing {required}: {markdown}"
        );
    }
    let mut reversed = fixture.proposal_command();
    let Command::CorrectionSeries { assessments, .. } = &mut reversed else {
        panic!("series command")
    };
    assessments.reverse();
    let replay = execute_with_clock(&reversed, &fixture.root, &NeverClock);
    assert_eq!(
        replay.to_json(),
        result.to_json(),
        "selected parent order is canonical"
    );
    let args = vec![
        "forecast".into(),
        "journal".into(),
        "correction".into(),
        "--journal".into(),
        JOURNAL.into(),
        "--head".into(),
        fixture.head.to_hex(),
        "--assessments".into(),
        fixture
            .parents
            .iter()
            .map(Digest32::to_hex)
            .collect::<Vec<_>>()
            .join(","),
        "--forecast".into(),
        "raw".into(),
    ];
    assert_eq!(
        fmine_cli::invoke(&args, &fixture.root).0.to_json(),
        result.to_json()
    );
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn reviewed_public_correction_freezes_resolves_and_replays_after_raw_outcomes() {
    let mut fixture = Fixture::series(true, 0, true);
    let proposal = fixture.proposal();
    let expected = digest_field(correction(&proposal.payload_json), "digest");
    let before = fixture.inspect();
    let unchanged_raw = forecast(&before.payload_json, "raw").to_string();
    let prior_bytes = fixture.bytes();
    let registered = fixture.ok(
        fixture.register_command(expected),
        ns("2032-02-06T00:00:00Z"),
    );
    assert!(
        fixture.bytes().starts_with(&prior_bytes),
        "raw history remains a byte-identical prefix"
    );
    assert_eq!(forecast(&registered.payload_json, "raw"), unchanged_raw);
    let corrected = forecast(&registered.payload_json, "corrected");
    assert_eq!(string_field(corrected, "model_key"), CORRECTED_MODEL);
    assert_eq!(string_field(corrected, "author_role"), "review-role");
    assert_eq!(
        string_field(corrected, "registered_at"),
        "2032-02-06T00:00:00Z"
    );
    assert_eq!(string_field(corrected, "outcome_due"), TARGET_DUE);
    assert!((number_field(corrected, "mean") - 100.0).abs() < 1e-10);
    assert!((number_field(corrected, "std_dev") - 2.0).abs() < 1e-10);
    for field in ["family", "domain", "horizon", "site", "quantity", "unit"] {
        assert_eq!(
            string_field(corrected, field),
            string_field(&unchanged_raw, field)
        );
    }
    let corrected_id = string_field(corrected, "forecast_id").to_string();
    assert_ne!(corrected_id, string_field(&unchanged_raw, "forecast_id"));
    assert!(corrected.contains("\"correction\":"));
    let admission = object(
        registered
            .payload_json
            .split_once("\"correction_admissions\":[")
            .unwrap()
            .1,
    )
    .to_string();
    assert!(admission.contains(&expected.to_hex()));
    assert!(
        admission.contains(correction(&proposal.payload_json)),
        "exact reviewed proposal retained"
    );
    let operation = {
        let durable =
            DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
        let Entry::Operation(operation) = &durable.records().last().unwrap().entry else {
            panic!("registered operation")
        };
        operation.clone()
    };
    assert!(operation.contains("schema = \"fmine.forecast-operation.v11\""));
    assert!(operation.contains("[[corrected_forecast]]"));
    assert!(operation.contains(&expected.to_hex()));

    fixture.append(
        &fixture.cohort("future-corrected", "corrected", 2032),
        ns("2032-02-07T00:00:00Z"),
    );
    fixture.resolve("future-corrected", "corrected", 2032, 100);
    let saved = fixture.ok(
        Command::Assess {
            journal: JOURNAL.into(),
            head: fixture.head,
            cohort: "future-corrected".into(),
            as_of: "2033-02-03T00:00:00Z".into(),
            selector: "first-published".into(),
            expected_digest: None,
        },
        ns("2033-02-03T00:00:00Z"),
    );
    let scored = assessment(&saved.payload_json).to_string();
    assert!(scored.contains(&corrected_id));
    assert!(scored.contains("\"value\":100,"));
    assert!(scored.contains("\"inside\":1"));
    assert!(scored.contains("\"source_admission\":{") && scored.contains("\"correction\":{"));
    assert!(scored.contains(&fixture.source_hashes[2].to_hex()));
    let assessed_digest = digest_field(&scored, "digest");

    // Resolving the unchanged raw target must not refit or invalidate a correction admitted earlier.
    fixture.resolve_at("future-raw", "raw", 2032, 100, ns("2033-02-04T00:00:00Z"));
    let mut paired = Vec::new();
    for (index, cohort) in ["future-raw", "future-corrected"].into_iter().enumerate() {
        let saved = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: cohort.into(),
                as_of: "2033-02-05T00:00:00Z".into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns("2033-02-05T00:00:00Z") + i64::try_from(index).unwrap(),
        );
        paired.push(digest_field(assessment(&saved.payload_json), "digest"));
    }
    let comparison_command = Command::Compare {
        journal: JOURNAL.into(),
        head: Some(fixture.head),
        left: paired[0],
        right: paired[1],
    };
    let before_comparison = fixture.bytes();
    let compared = execute_with_clock(&comparison_command, &fixture.root, &NeverClock);
    assert_eq!(compared.exit_code, 0, "{}", compared.to_json());
    assert_eq!(fixture.bytes(), before_comparison);
    let parsed = parse(compared.payload_json.as_bytes()).unwrap();
    let comparison = parsed.get("comparison").unwrap();
    assert_eq!(comparison.get("n_members").unwrap().f64(), Some(1.0));
    assert_eq!(comparison.get("n_paired").unwrap().f64(), Some(1.0));
    let difference = comparison.get("observed_mean_difference").unwrap();
    assert!(
        difference
            .get("num")
            .unwrap()
            .as_str()
            .unwrap()
            .parse::<i128>()
            .unwrap()
            > 0,
        "raw mean 50 misses actual 100 while the separately registered corrected mean 100 covers it"
    );
    assert_eq!(
        comparison
            .get("whole_population")
            .unwrap()
            .get("lower")
            .unwrap(),
        difference
    );
    assert_eq!(
        comparison
            .get("whole_population")
            .unwrap()
            .get("upper")
            .unwrap(),
        difference
    );
    assert!(
        compared
            .payload_json
            .contains(&fixture.source_hashes[2].to_hex())
    );
    for year in [2030, 2031, 2032] {
        fs::write(
            fixture.root.join(format!("corpus/{year}Q4.xlsx")),
            b"changed after recorded admissions",
        )
        .unwrap();
    }
    let reopened = fixture.inspect();
    parse(reopened.payload_json.as_bytes()).unwrap();
    assert_eq!(forecast(&reopened.payload_json, "raw"), unchanged_raw);
    assert_eq!(
        object(
            reopened
                .payload_json
                .split_once("\"correction_admissions\":[")
                .unwrap()
                .1
        ),
        admission,
        "replay retains the historical proposal and source policies after future outcomes",
    );
    let retrieved = execute_with_clock(
        &Command::Report {
            journal: JOURNAL.into(),
            head: Some(fixture.head),
            assessment: assessed_digest,
            provenance: None,
            timestamp: None,
        },
        &fixture.root,
        &NeverClock,
    );
    assert_eq!(retrieved.exit_code, 0, "{}", retrieved.to_json());
    assert_eq!(assessment(&retrieved.payload_json), scored);
    assert_eq!(
        execute_with_clock(&comparison_command, &fixture.root, &NeverClock).to_json(),
        compared.to_json(),
        "saved raw-versus-corrected comparison does not reread changed source files",
    );
    for (parent, original) in fixture.parents.iter().zip(&fixture.saved_assessments) {
        let retrieved = execute_with_clock(
            &Command::Report {
                journal: JOURNAL.into(),
                head: Some(fixture.head),
                assessment: *parent,
                provenance: None,
                timestamp: None,
            },
            &fixture.root,
            &NeverClock,
        );
        assert_eq!(retrieved.exit_code, 0, "{}", retrieved.to_json());
        assert_eq!(assessment(&retrieved.payload_json), original);
    }
}

#[test]
fn public_corrected_forecast_cannot_escape_inherited_source_policy() {
    let mut fixture = Fixture::series(true, 0, true);
    let expected = digest_field(correction(&fixture.proposal().payload_json), "digest");
    fixture.ok(
        fixture.register_command(expected),
        ns("2032-02-06T00:00:00Z"),
    );
    // The inherited public binding applies immediately, before the new cohort has been frozen.
    let manual = fixture.request(&supplied_outcome("corrected", 100, 2032));
    fixture.refuse(&manual, ns("2033-02-02T00:00:00Z"));
    let proper = fixture.cohort("future-corrected", "corrected", 2032);
    let generic = "[[cohort]]\nname = \"future-corrected\"\nmembers = \"corrected\"\nresolution_rule = \"Synthetic annual output on a fixed basis\"\neligible_sources = \"synthetic:issuer\"\nalpha = \"0.2\"\n";
    let alternatives = [
        generic.to_string(),
        proper.replace("alpha = \"0.2\"", "alpha = \"0.1\""),
        proper.replace(
            "public_ownership = \"100%\"",
            "public_ownership = \"attributable:3/5\"",
        ),
        format!("{proper}public_vintages = \"registered_comparatives\"\n"),
    ];
    for altered in alternatives {
        let command = fixture.request(&altered);
        fixture.refuse(&command, ns("2032-02-07T00:00:00Z"));
    }
    fixture.append(&proper, ns("2032-02-07T00:00:00Z"));
    let manual = fixture.request(&supplied_outcome("corrected", 100, 2032));
    fixture.refuse(&manual, ns("2033-02-02T00:00:00Z"));
    fixture.resolve("future-corrected", "corrected", 2032, 100);
}

#[test]
fn correction_registration_checks_review_identity_head_and_clock_upper_boundary() {
    let mut fixture = Fixture::series(false, SECOND as u64, true);
    let expected = digest_field(correction(&fixture.proposal().payload_json), "digest");
    for change in [
        "digest",
        "head",
        "alias",
        "model",
        "parent",
        "selection",
        "duplicate",
    ] {
        let mut command = fixture.register_command(expected);
        let Command::RegisterCorrection {
            head,
            id,
            model_key,
            expected_proposal,
            assessments,
            series,
            ..
        } = &mut command
        else {
            panic!("registration command")
        };
        match change {
            "digest" => *expected_proposal = Digest32([9; 32]),
            "head" => *head = Digest32([8; 32]),
            "alias" => *id = "raw".into(),
            "model" => *model_key = RAW_MODEL.into(),
            "parent" => assessments[0] = Digest32([7; 32]),
            "selection" => *series = false,
            "duplicate" => assessments[1] = assessments[0],
            _ => unreachable!(),
        }
        fixture.refuse(&command, ns("2032-02-06T00:00:00Z"));
    }
    let valid = fixture.register_command(expected);
    let equal_upper = ns(TARGET_DUE) - SECOND;
    fixture.refuse(&valid, equal_upper);
    let result = fixture.ok(valid, equal_upper - 1);
    let registered = forecast(&result.payload_json, "corrected");
    assert!(registered.contains(&format!("\"registered_ns\":{},", ns(TARGET_DUE) - 1)));
    assert!(registered.contains(&format!("\"outcome_due_ns\":{},", ns(TARGET_DUE))));
    assert_eq!(
        number_field(forecast(&result.payload_json, "raw"), "mean"),
        50.0
    );
}

#[test]
fn prospective_registration_requires_a_frozen_raw_resolution_policy() {
    let fixture = Fixture::series(false, 0, false);
    let proposal = fixture.proposal();
    let expected = digest_field(correction(&proposal.payload_json), "digest");
    assert!(correction(&proposal.payload_json).contains("\"target_cohorts\":[]"));
    fixture.refuse(
        &fixture.register_command(expected),
        ns("2032-02-06T00:00:00Z"),
    );
}

#[test]
fn single_saved_assessment_can_issue_its_unchanged_cor1_proposal() {
    let mut fixture = Fixture::single();
    let proposal = fixture.proposal();
    let original = correction(&proposal.payload_json);
    assert!(original.contains("\"schema\":\"fmine.forecast-correction.v1\""));
    assert!(!original.contains("COR2"));
    let expected = digest_field(original, "digest");
    let registered = fixture.ok(
        fixture.register_command(expected),
        ns("2032-02-06T00:00:00Z"),
    );
    assert!(registered.payload_json.contains(original));
    assert!(
        (number_field(forecast(&registered.payload_json, "corrected"), "mean") - 100.0).abs()
            < 1e-10
    );
    assert_eq!(
        forecast(&registered.payload_json, "raw"),
        forecast(&fixture.inspect().payload_json, "raw")
    );
}

#[test]
fn parser_requires_one_selection_mode_and_explicit_reviewed_registration() {
    let fixture = Fixture::series(false, 0, true);
    let before = fixture.bytes();
    let expected = digest_field(correction(&fixture.proposal().payload_json), "digest");
    let base = vec![
        "forecast".into(),
        "journal".into(),
        "register-correction".into(),
        "--journal".into(),
        JOURNAL.into(),
        "--head".into(),
        fixture.head.to_hex(),
        "--id".into(),
        "corrected".into(),
        "--forecast".into(),
        "raw".into(),
        "--assessments".into(),
        fixture
            .parents
            .iter()
            .map(Digest32::to_hex)
            .collect::<Vec<_>>()
            .join(","),
        "--expected-proposal".into(),
        expected.to_hex(),
        "--model-key".into(),
        CORRECTED_MODEL.into(),
        "--author-role".into(),
        "review-role".into(),
        "--trust-host-clock".into(),
    ];
    let (parsed, _, _) = parse_args(&base, &fixture.root).unwrap();
    let CliCommand::ForecastJournal(parsed) = parsed else {
        panic!("durable forecast command")
    };
    assert_eq!(parsed, fixture.register_command(expected));
    for flag in [
        "--head",
        "--expected-proposal",
        "--model-key",
        "--author-role",
        "--assessments",
    ] {
        let mut missing = base.clone();
        let index = missing.iter().position(|word| word == flag).unwrap();
        missing.drain(index..=index + 1);
        assert!(
            parse_args(&missing, &fixture.root).is_err(),
            "missing {flag}"
        );
    }
    let mut untrusted = base.clone();
    untrusted.retain(|word| word != "--trust-host-clock");
    assert!(parse_args(&untrusted, &fixture.root).is_err());
    let mut mixed = base.clone();
    mixed.extend(["--assessment".into(), fixture.parents[0].to_hex()]);
    assert!(parse_args(&mixed, &fixture.root).is_err());
    let mut one = base.clone();
    let at = one.iter().position(|word| word == "--assessments").unwrap() + 1;
    one[at] = fixture.parents[0].to_hex();
    assert!(parse_args(&one, &fixture.root).is_err());
    assert_eq!(
        fixture.bytes(),
        before,
        "parser has no journal mutation authority"
    );
}

#[test]
fn intact_frame_chains_cannot_rewrite_correction_semantics_or_downgrade_its_version() {
    let mut fixture = Fixture::series(false, 0, true);
    let expected = digest_field(correction(&fixture.proposal().payload_json), "digest");
    let registered = fixture.ok(
        fixture.register_command(expected),
        ns("2032-02-06T00:00:00Z"),
    );
    let wrong_raw = string_field(
        forecast(&registered.payload_json, "train-2030"),
        "forecast_id",
    );
    let (header, records) = {
        let source =
            DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
        (source.header().clone(), source.records().to_vec())
    };
    for change in [
        "unchanged",
        "mean_bits",
        "raw_forecast_id",
        "target_cohorts",
        "assessments",
        "missing-schema",
        "old-schema",
    ] {
        let path = fixture.root.join(format!("replay-{change}.journal"));
        let mut writer = DurableJournal::create(
            &path,
            header.cell_id.clone(),
            header.clock_skew_ns,
            &FixedClock(records[0].observed_ns),
        )
        .unwrap();
        let mut changed_rows = 0;
        for record in &records[1..] {
            let Entry::Operation(original) = &record.entry else {
                panic!("operation after genesis")
            };
            let operation = if original.contains("[[corrected_forecast]]") {
                let mut operation = String::new();
                for line in original.lines() {
                    let replacement = match change {
                        "mean_bits" if line.starts_with("mean_bits = ") => {
                            format!("mean_bits = \"{:016x}\"", 1.0_f64.to_bits())
                        }
                        "raw_forecast_id" if line.starts_with("raw_forecast_id = ") => {
                            format!("raw_forecast_id = \"{wrong_raw}\"")
                        }
                        "target_cohorts" if line.starts_with("target_cohorts = ") => {
                            format!("target_cohorts = \"{}\"", Digest32([9; 32]))
                        }
                        "assessments" if line.starts_with("assessments = ") => format!(
                            "assessments = \"{},{}\"",
                            fixture.parents[0], fixture.parents[0]
                        ),
                        "missing-schema" if line.starts_with("schema = ") => {
                            changed_rows += 1;
                            continue;
                        }
                        "old-schema" if line.starts_with("schema = ") => {
                            "schema = \"fmine.forecast-operation.v2\"".into()
                        }
                        _ => line.into(),
                    };
                    changed_rows += usize::from(replacement != line);
                    operation.push_str(&replacement);
                    operation.push('\n');
                }
                operation
            } else {
                original.clone()
            };
            // Intentionally use the storage-only callback: every resulting frame/hash is valid,
            // leaving the production semantic replay responsible for detecting the altered record.
            writer
                .append(&operation, &FixedClock(record.observed_ns), |_, _| Ok(()))
                .unwrap();
        }
        assert_eq!(changed_rows, usize::from(change != "unchanged"));
        let head = writer.head();
        drop(writer);
        let intact = DurableJournal::open_read(&path, Some(head)).unwrap();
        assert_eq!(intact.records().len(), records.len());
        drop(intact);
        let before = fs::read(&path).unwrap();
        let reopened = execute_with_clock(
            &Command::Inspect {
                journal: path.clone(),
                head: Some(head),
            },
            &fixture.root,
            &NeverClock,
        );
        if change == "unchanged" {
            assert_eq!(reopened.exit_code, 0, "{}", reopened.to_json());
            assert_eq!(
                forecast(&reopened.payload_json, "corrected"),
                forecast(&registered.payload_json, "corrected")
            );
        } else {
            assert_ne!(reopened.exit_code, 0, "{change}: {}", reopened.to_json());
            let refusal = reopened.refusal.as_ref().unwrap();
            let reason = match change {
                "mean_bits" | "raw_forecast_id" | "target_cohorts" => "stored correction payload",
                "assessments" => "repeats a saved assessment",
                _ => "prospective correction registration",
            };
            assert!(refusal.what.contains(reason), "{change}: {refusal:?}");
        }
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "replay never repairs or truncates a refused journal"
        );
    }
}
