//! Source-bound forecast admissions through registered XLSX bytes and the durable CLI journal.
//! Every issuer, workbook, publication and clock below is invented test evidence, not a real filing.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{parse_hex32, parse_utc};
use fmine_claim::forecast_journal::Entry;
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{CliCommand, Envelope, parse_args};
use fmine_core::hash::{Digest32, sha256};
use fmine_io::forecast::Journal as DurableJournal;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};
use fmine_public::key::KeyedCell;
use fmine_public::sources::{load_issuer_profile, registry_path};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_runtime::{ClockError, WallClock};

const ISSUER: &str = "example";
const JOURNAL: &str = "forecast.journal";
const COHORT: &str = "registered-site";
const AVAILABLE: &str = "2031-02-16T00:00:00Z";
const CUT: &str = "2031-02-17T00:00:00Z";
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
        panic!("reading a persisted assessment must not sample a clock")
    }
}

fn ns(value: &str) -> i64 {
    parse_utc(value)
        .unwrap()
        .checked_mul(1_000_000_000)
        .unwrap()
}

fn string_field<'a>(json: &'a str, name: &str) -> &'a str {
    json.split_once(&format!("\"{name}\":\""))
        .unwrap_or_else(|| panic!("missing {name}: {json}"))
        .1
        .split('"')
        .next()
        .unwrap()
}

fn digest_field(json: &str, name: &str) -> Digest32 {
    Digest32(parse_hex32(string_field(json, name)).unwrap())
}

fn assessment(json: &str) -> &str {
    json.split_once("\"assessment\":")
        .unwrap()
        .1
        .split_once(",\"checkpoints\":")
        .unwrap()
        .0
}

fn sheet(name: &str, months: &str, production: &str, cost: &str, end: &str) -> MinimalSheet {
    let text = |value: &str| MinimalCell::Text(value.into());
    let number = |value: &str| MinimalCell::Styled {
        raw: value.into(),
        xf: 0,
    };
    MinimalSheet {
        name: name.into(),
        hidden: false,
        cells: vec![
            (
                "A3".into(),
                text(&format!("For the {months} months ended {end},")),
            ),
            ("B3".into(), number("2030")),
            ("A5".into(), text("Site A (60%)")),
            ("B4".into(), text("Gold produced (000s oz 100%)")),
            ("B5".into(), number(production)),
            ("C4".into(), text("Gold produced (000s oz attributable)")),
            ("C5".into(), number("240.4")),
            ("D4".into(), text(cost)),
            ("D5".into(), number("1000.4")),
        ],
    }
}

fn workbook(root: &Path, release: &str, published: Option<&str>) -> Digest32 {
    workbook_with_labels(
        root,
        release,
        published,
        "All-in sustaining costs ($/oz)",
        "December 31",
    )
}

fn workbook_with_labels(
    root: &Path,
    release: &str,
    published: Option<&str>,
    cost: &str,
    end: &str,
) -> Digest32 {
    let bytes = build_minimal_xlsx_styled(
        &[
            sheet("YTD", "twelve", "400.4", cost, end),
            sheet("QTD", "three", "100.4", cost, end),
        ],
        Some(&MinimalStyles {
            num_fmts: vec![],
            xfs: vec![1],
        }),
    );
    let digest = sha256(&bytes);
    let file = format!("{release}.xlsx");
    fs::write(root.join("corpus").join(&file), &bytes).unwrap();
    let date = published.map_or_else(String::new, |date| {
        format!(
            "published_at = \"{date}\"\npublished_at_basis = \"FACT: invented publication fixture\"\n"
        )
    });
    fs::write(
        registry_path(root, ISSUER),
        format!(
            "schema = \"fmine.sources.v1\"\nissuer = \"{ISSUER}\"\ncorpus_dir = \"corpus\"\nretrieved_utc = \"2031-03-01T00:00:00Z\"\nurl_prefix = \"https://example.invalid/\"\n\n[[subject]]\nname = \"Site A\"\n\n[[source]]\nfile = \"{file}\"\nkind = \"mine_stats\"\nquarter = \"{release}\"\npath = \"{file}\"\nurl_basis = \"W1 table\"\nbytes = {}\nsha256 = \"{}\"\n{date}",
            bytes.len(), digest.to_hex()
        ),
    )
    .unwrap();
    digest
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    source_digest: Digest32,
}

impl Fixture {
    fn new(metric: &str, ownership: &str, period: &str, lo: i64, hi: i64) -> Self {
        let root = std::env::temp_dir().join(format!(
            "fmine-registered-outcome-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for directory in ["corpus", "registries/sources"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        let source_digest = workbook(&root, "2030Q4", Some("2031-02-15"));
        let mut fixture = Self {
            root,
            head: Digest32::default(),
            source_digest,
        };
        fixture.ok(
            Command::Init {
                journal: JOURNAL.into(),
                cell_id: "invented-source-test".into(),
                clock_skew_ns: 0,
            },
            ns("2030-01-01T00:00:00Z"),
        );
        let forecast = fixture.request(
            "register.toml",
            &format!(
                "[[forecast]]\nid = \"one\"\nmodel_key = \"invented-fixed-target\"\noutcome_due = \"2030-12-31T23:59:59Z\"\nkind = \"interval\"\nlo = \"{lo}\"\nhi = \"{hi}\"\nnominal = \"0.8\"\n"
            ),
        );
        fixture.ok(
            Command::Register {
                journal: JOURNAL.into(),
                head: fixture.head,
                request: forecast,
            },
            ns("2030-01-02T00:00:00Z"),
        );
        let freeze = fixture.request(
            "freeze.toml",
            &format!(
                "[[cohort]]\nname = \"{COHORT}\"\nmembers = \"one\"\nresolution_rule = \"registered-public-cell\"\neligible_sources = \"{ISSUER}\"\npublic_issuer = \"{ISSUER}\"\npublic_metric = \"{metric}\"\npublic_period = \"{period}\"\npublic_ownership = \"{ownership}\"\npublic_members = \"one=Site A\"\n"
            ),
        );
        fixture.ok(
            Command::Freeze {
                journal: JOURNAL.into(),
                head: fixture.head,
                request: freeze,
            },
            ns("2030-01-03T00:00:00Z"),
        );
        fixture
    }

    fn annual(ownership: &str) -> Self {
        Self::new("production_koz", ownership, "2030FY", 350, 450)
    }

    fn request(&self, name: &str, text: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join(JOURNAL)).unwrap()
    }

    fn ok(&mut self, command: Command, at: i64) -> Envelope {
        let envelope = execute_with_clock(&command, &self.root, &FixedClock(at));
        assert_eq!(envelope.exit_code, 0, "{envelope:?}");
        self.head = digest_field(&envelope.payload_json, "head");
        envelope
    }

    fn resolve(&self) -> Command {
        Command::Resolve {
            journal: JOURNAL.into(),
            head: self.head,
            cohort: COHORT.into(),
            forecast: "one".into(),
        }
    }

    fn refuse(&self, command: &Command, at: i64, code: &str) {
        let before = self.bytes();
        let envelope = execute_with_clock(command, &self.root, &FixedClock(at));
        assert_ne!(envelope.exit_code, 0, "{envelope:?}");
        assert_eq!(envelope.refusal.unwrap().code, code);
        assert_eq!(self.bytes(), before, "refusal must not append any bytes");
        let journal = DurableJournal::open_read(&self.root.join(JOURNAL), Some(self.head));
        assert!(journal.is_ok(), "original head must still verify");
    }

    fn source_claim(&self, locator: &str) -> KeyedCell {
        let profile = load_issuer_profile(&self.root, ISSUER).unwrap();
        fmine_public::key::mine_stats::load_claims(&profile, &self.root)
            .unwrap()
            .claims
            .into_iter()
            .find(|cell| cell.locator == locator)
            .unwrap()
    }
}

#[test]
fn original_registered_cells_resolve_exact_printed_values_and_reopen_their_assessments() {
    for (metric, ownership, period, locator, printed, lo, hi) in [
        (
            "production_koz",
            "100%",
            "2030FY",
            "2030Q4.xlsx:YTD!B5",
            400,
            350,
            450,
        ),
        (
            "production_koz",
            "attributable:3/5",
            "2030FY",
            "2030Q4.xlsx:YTD!C5",
            240,
            200,
            300,
        ),
        (
            "production_koz",
            "100%",
            "2030Q4",
            "2030Q4.xlsx:QTD!B5",
            100,
            90,
            110,
        ),
        (
            "aisc_usd_per_oz",
            "attributable:3/5",
            "2030FY",
            "2030Q4.xlsx:YTD!D5",
            1000,
            900,
            1100,
        ),
    ] {
        let mut fixture = Fixture::new(metric, ownership, period, lo, hi);
        let source = fixture.source_claim(locator);
        assert!(
            source
                .to_json()
                .contains(&format!("\"stored\":\"{printed}.4\""))
        );
        let envelope = fixture.ok(fixture.resolve(), ns(AVAILABLE));
        let json = envelope.to_json();
        for expected in [
            "\"n_outcomes\":1".to_string(),
            "\"operation_version_counts\":{\"legacy_v1\":0,\"v2\":1,\"v3\":2}".to_string(),
            format!("\"source_claim_id\":\"{}\"", source.claim.claim_id),
            format!("\"exact_value\":{{\"numerator\":\"{printed}\",\"denominator\":\"1\"}}"),
            "\"published_at\":\"2031-02-15\"".to_string(),
            "\"source_bytes_reverified_on_replay\":false".to_string(),
            "\"external_time_proof\":\"not_verified\"".to_string(),
        ] {
            assert!(json.contains(&expected), "missing {expected}: {json}");
        }
        let persisted =
            DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
        let operation = persisted
            .records()
            .iter()
            .find_map(|record| {
                if let Entry::Operation(text) = &record.entry {
                    let doc = parse_subset(text).unwrap();
                    doc.rows
                        .into_iter()
                        .find(|row| row.table == "registered_outcome")
                } else {
                    None
                }
            })
            .unwrap();
        for (field, expected) in [
            ("value", printed.to_string()),
            ("claim_id", source.claim.claim_id.to_string()),
            ("sha256", fixture.source_digest.to_hex()),
            ("locator", locator.to_string()),
            ("available_at", AVAILABLE.to_string()),
        ] {
            assert_eq!(
                operation.get(field).map(|field| &field.value),
                Some(&TomlValue::Str(expected))
            );
        }
        drop(persisted);
        let assessed = fixture.ok(
            Command::Assess {
                journal: JOURNAL.into(),
                head: fixture.head,
                cohort: COHORT.into(),
                as_of: CUT.into(),
                selector: "first-published".into(),
                expected_digest: None,
            },
            ns(CUT),
        );
        assert!(
            assessed
                .payload_json
                .contains("\"operation_version_counts\":{\"legacy_v1\":0,\"v2\":2,\"v3\":2}")
        );
        let expected = assessment(&assessed.payload_json);
        assert!(expected.contains("\"n_members\":1,\"n_resolved\":1,\"n_unresolved\":0"));
        assert!(expected.contains(&format!("\"status\":\"resolved\",\"value\":{printed},")));
        assert!(expected.contains(&format!("\"observed_mean\":{},", hi - lo)));
        assert!(expected.contains(locator));
        assert!(expected.contains(&source.claim.claim_id.to_string()));
        let saved = digest_field(expected, "digest");
        let before = fixture.bytes();
        fs::rename(
            fixture.root.join("corpus/2030Q4.xlsx"),
            fixture.root.join("corpus/2030Q4.xlsx.saved"),
        )
        .unwrap();
        let reopened = execute_with_clock(
            &Command::Report {
                journal: JOURNAL.into(),
                head: Some(fixture.head),
                assessment: saved,
                provenance: None,
                timestamp: None,
            },
            &fixture.root,
            &NeverClock,
        );
        assert_eq!(reopened.exit_code, 0, "{reopened:?}");
        assert_eq!(assessment(&reopened.payload_json), expected);
        assert_eq!(fixture.bytes(), before);
        assert!(
            reopened
                .to_json()
                .contains("\"source_bytes_reverified_on_replay\":false")
        );
    }
}

#[test]
fn wrong_basis_changed_bytes_and_missing_original_never_append_an_outcome() {
    let wrong_basis = Fixture::annual("attributable:1/2");
    wrong_basis.refuse(&wrong_basis.resolve(), ns(AVAILABLE), "FMINE-REF-105");

    let altered = Fixture::annual("100%");
    let path = altered.root.join("corpus/2030Q4.xlsx");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(0);
    fs::write(path, bytes).unwrap();
    altered.refuse(&altered.resolve(), ns(AVAILABLE), "FMINE-REF-505");

    let missing = Fixture::annual("100%");
    fs::rename(
        missing.root.join("corpus/2030Q4.xlsx"),
        missing.root.join("corpus/2030Q4.xlsx.saved"),
    )
    .unwrap();
    missing.refuse(&missing.resolve(), ns(AVAILABLE), "FMINE-REF-506");

    let comparative = Fixture::annual("100%");
    workbook(&comparative.root, "2031Q1", Some("2031-05-01"));
    comparative.refuse(
        &comparative.resolve(),
        ns("2031-05-02T00:00:00Z"),
        "FMINE-REF-201",
    );
}

#[test]
fn registered_headers_cannot_reinterpret_the_frozen_cost_or_calendar_period() {
    let cost = Fixture::new("aisc_usd_per_oz", "attributable:3/5", "2030FY", 900, 1100);
    workbook_with_labels(
        &cost.root,
        "2030Q4",
        Some("2031-02-15"),
        "All-in sustaining costs ($/oz) per produced ounce",
        "December 31",
    );
    cost.refuse(&cost.resolve(), ns(AVAILABLE), "FMINE-REF-105");

    for period in ["2030FY", "2030Q4"] {
        let shifted = Fixture::new("production_koz", "100%", period, 0, 500);
        workbook_with_labels(
            &shifted.root,
            "2030Q4",
            Some("2031-02-15"),
            "All-in sustaining costs ($/oz)",
            "June 30",
        );
        shifted.refuse(&shifted.resolve(), ns(AVAILABLE), "FMINE-REF-102");
    }
}

#[test]
fn publication_day_and_missing_publication_evidence_cannot_be_backdated() {
    let mut fixture = Fixture::annual("100%");
    fixture.refuse(&fixture.resolve(), ns(AVAILABLE) - 1, "FMINE-REF-206");
    fixture.ok(fixture.resolve(), ns(AVAILABLE));

    let undated = Fixture::annual("100%");
    workbook(&undated.root, "2030Q4", None);
    undated.refuse(&undated.resolve(), ns(AVAILABLE), "FMINE-REF-202");

    let premature = Fixture::annual("100%");
    workbook(&premature.root, "2030Q4", Some("2030-12-30"));
    premature.refuse(&premature.resolve(), ns(AVAILABLE), "FMINE-REF-108");
}

#[test]
fn caller_values_and_copied_receipts_cannot_bypass_the_frozen_source_policy() {
    let mut fixture = Fixture::annual("100%");
    let request = fixture.request("supplied.toml", &format!(
        "[[outcome]]\nforecast = \"one\"\nkind = \"published\"\navailable_at = \"{AVAILABLE}\"\nvalue = \"999\"\nsource = \"{ISSUER}\"\nvintage = \"2030Q4\"\nlocator = \"2030Q4.xlsx:YTD!B5\"\nsha256 = \"{}\"\n",
        fixture.source_digest.to_hex()
    ));
    fixture.refuse(
        &Command::Outcome {
            journal: JOURNAL.into(),
            head: fixture.head,
            request,
        },
        ns(AVAILABLE),
        "FMINE-REF-107",
    );
    fixture.ok(fixture.resolve(), ns(AVAILABLE));
    let persisted =
        DurableJournal::open_read(&fixture.root.join(JOURNAL), Some(fixture.head)).unwrap();
    let receipt = persisted
        .records()
        .iter()
        .find_map(|record| match &record.entry {
            Entry::Operation(text) if text.contains("[[registered_outcome]]") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    drop(persisted);
    let client = receipt
        .lines()
        .filter(|line| !line.starts_with("schema ="))
        .collect::<Vec<_>>()
        .join("\n");
    let request = fixture.request("copied-receipt.toml", &client);
    fixture.refuse(
        &Command::Outcome {
            journal: JOURNAL.into(),
            head: fixture.head,
            request,
        },
        ns(CUT),
        "FMINE-REF-107",
    );
}

#[test]
fn replay_rejects_a_changed_value_even_in_an_intact_journal_frame() {
    let mut original = Fixture::annual("100%");
    original.ok(original.resolve(), ns(AVAILABLE));
    let journal =
        DurableJournal::open_read(&original.root.join(JOURNAL), Some(original.head)).unwrap();
    let receipt = journal
        .records()
        .iter()
        .find_map(|record| match &record.entry {
            Entry::Operation(text) if text.contains("[[registered_outcome]]") => Some(text.clone()),
            _ => None,
        })
        .unwrap();
    drop(journal);
    assert!(receipt.starts_with("schema = \"fmine.forecast-operation.v3\""));
    let mut changed = receipt.clone();
    for field in ["value", "numerator"] {
        let from = format!("{field} = \"400\"");
        assert_eq!(changed.matches(&from).count(), 1);
        changed = changed.replace(&from, &format!("{field} = \"401\""));
    }
    for field in ["claim_id =", "claim_body ="] {
        assert_eq!(
            receipt.lines().find(|line| line.starts_with(field)),
            changed.lines().find(|line| line.starts_with(field))
        );
    }

    for (operation, should_refuse) in [(&receipt, false), (&changed, true)] {
        // Each recipient has the same frozen target and no outcome, so a duplicate publication
        // cannot hide failure of the retained source-evidence check. The opaque I/O adapter writes
        // a valid frame/hash chain; semantic acceptance is tested independently by CLI replay.
        let mut recipient = Fixture::annual("100%");
        let mut writer =
            DurableJournal::open_write(&recipient.root.join(JOURNAL), recipient.head).unwrap();
        writer
            .append(operation, &FixedClock(ns(AVAILABLE)), |_, _| Ok(()))
            .unwrap();
        recipient.head = writer.head();
        drop(writer);
        let before = recipient.bytes();
        let result = execute_with_clock(
            &Command::Inspect {
                journal: JOURNAL.into(),
                head: Some(recipient.head),
            },
            &recipient.root,
            &NeverClock,
        );
        if should_refuse {
            let refusal = result
                .refusal
                .expect("unchanged source Claim cannot attest to401");
            assert_eq!(refusal.code, "FMINE-REF-507");
            assert!(
                refusal.what.contains("source Claim does not bind"),
                "{refusal:?}"
            );
        } else {
            assert_eq!(result.exit_code, 0, "{result:?}");
            assert!(result.payload_json.contains("\"n_outcomes\":1"));
        }
        assert_eq!(recipient.bytes(), before, "replay is read-only");
    }
}

#[test]
fn resolve_cli_uses_only_the_frozen_cohort_member_and_explicit_head() {
    let fixture = Fixture::annual("100%");
    let words = [
        "forecast",
        "journal",
        "resolve",
        "--journal",
        JOURNAL,
        "--head",
        &fixture.head.to_hex(),
        "--cohort",
        COHORT,
        "--forecast",
        "one",
        "--trust-host-clock",
    ];
    let args: Vec<_> = words.iter().map(|word| (*word).to_string()).collect();
    let (command, _, root) = parse_args(&args, &fixture.root).unwrap();
    assert_eq!(root, fixture.root);
    assert_eq!(command, CliCommand::ForecastJournal(fixture.resolve()));
    for extra in [
        ["--value", "999"],
        ["--issuer", "other"],
        ["--request", "supplied.toml"],
        ["--public-period", "2031FY"],
    ] {
        let mut changed = args.clone();
        changed.extend(extra.into_iter().map(str::to_string));
        assert!(parse_args(&changed, &fixture.root).is_err());
    }
    let no_trust: Vec<_> = args
        .iter()
        .filter(|word| *word != "--trust-host-clock")
        .cloned()
        .collect();
    assert!(parse_args(&no_trust, &fixture.root).is_err());
}
