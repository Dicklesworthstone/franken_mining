//! Synthetic raw observations exercise plant forecast admission, scoring and immutable replay.
//! No fixture is a real plant measurement, independent source audit or external clock proof.
#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_calib::record::{format_utc, parse_hex32, parse_utc};
use fmine_claim::forecast_journal::{Entry, Record};
use fmine_cli::forecast_journal::{Command, execute_with_clock};
use fmine_cli::{CliCommand, Envelope};
use fmine_core::{Digest32, hash::sha256};
use fmine_io::forecast::Journal;
use fmine_io::omf::json::{Json, parse};
use fmine_runtime::{ClockError, WallClock};

const NS: i64 = 1_000_000_000;
const HOUR: i64 = 3_600 * NS;
const RULE: &str = "plant-two-product-recovery/v1";

struct Clock(i64);
impl WallClock for Clock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        Ok(self.0)
    }
}
struct NoClock;
impl WallClock for NoClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        panic!("read-only replay and pre-admission refusals must not sample time")
    }
}

fn field<'a>(value: &'a Json, path: &[&str]) -> &'a Json {
    path.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn utc(ns: i64) -> String {
    format_utc(ns / NS)
}

fn material(role: &str, dry: &str, assay: &str, start: i64) -> String {
    format!(
        "# retained-observation-marker-{role}\nschema = \"fmine.plant-material.v1\"\nboundary = \"synthetic-plant/v1\"\nmaterial = \"{role}\"\nstart_ns = \"{start}\"\nend_ns = \"{}\"\nassay_basis = \"total_element/all_non_water_dry_matter\"\nwhole_material_support = \"true\"\n\n[[mass]]\nrole = \"dry\"\nvalue = \"{dry}\"\nunit = \"kg\"\n\n[[mass]]\nrole = \"water\"\nvalue = \"0\"\nunit = \"kg\"\n\n[[assay]]\nelement = \"Au\"\nmeasurand = \"total-Au/all-phases\"\nvalue = \"{assay}\"\nunit = \"g_per_t\"\n",
        start + HOUR,
    )
}

struct Fixture {
    root: PathBuf,
    head: Digest32,
    start: i64,
}

impl Fixture {
    fn new(target: &str) -> Self {
        Self::with_dry_basis(target, "total-non-water-dry-matter/v1")
    }

    fn with_dry_basis(target: &str, dry_basis: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = loop {
            let path = std::env::temp_dir().join(format!(
                "fmine-plant-outcome-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("synthetic directory: {error}"),
            }
        };
        let start = parse_utc("2030-01-02T00:00:00Z").unwrap() * NS;
        for (name, text) in [
            (
                "production",
                include_str!("../../../tests/plant/production_joint.toml"),
            ),
            (
                "grinding",
                include_str!("../../../tests/plant/grinding_joint.toml"),
            ),
            (
                "recovery",
                include_str!("../../../tests/plant/production_recovery.toml"),
            ),
        ] {
            let text = text
                .replace("start_ns = \"0\"", &format!("start_ns = \"{start}\""))
                .replace(
                    "end_ns = \"3600000000000\"",
                    &format!("end_ns = \"{}\"", start + HOUR),
                )
                .replace("total-non-water-dry-matter/v1", dry_basis);
            fs::write(root.join(format!("{name}.toml")), text).unwrap();
        }
        let initialized = execute_with_clock(
            &Command::Init {
                journal: "journal".into(),
                cell_id: "synthetic-plant".into(),
                clock_skew_ns: NS as u64,
            },
            &root,
            &Clock(start - 60 * NS),
        );
        assert_eq!(initialized.exit_code, 0, "{}", initialized.to_json());
        let mut result = Self {
            root,
            head: Digest32::default(),
            start,
        };
        result.update_head(&initialized);
        result.ok(
            "register-plant",
            &[
                "--input",
                "production.toml",
                "--grinding",
                "grinding.toml",
                "--recovery",
                "recovery.toml",
                "--id",
                "prediction",
                "--target",
                target,
                "--author-role",
                "synthetic-model-role",
                "--time-basis",
                "utc-unix-ns",
            ],
            start - 50 * NS,
        );
        result.freeze("monthly", "prediction", RULE, start - 40 * NS);
        fs::create_dir(result.root.join("observations")).unwrap();
        for (role, dry, assay) in [
            ("feed", "1000", "10"),
            ("concentrate", "100", "80"),
            ("tailings", "900", "20/9"),
        ] {
            fs::write(
                result.root.join(format!("observations/{role}.toml")),
                material(role, dry, assay, start),
            )
            .unwrap();
        }
        fs::write(result.root.join("observations/observation.toml"), format!(
            "schema = \"fmine.plant-recovery-observation.v1\"\n\n[[observation]]\nsource = \"synthetic:plant-balance\"\nvintage = \"first-hour\"\navailable_at = \"{}\"\ncircuit = \"synthetic-ball-mill\"\nblend = \"synthetic-shift-blend\"\nmaterial_domain = \"invented-ore-domain\"\ndry_basis = \"total-non-water-dry-matter/v1\"\nelement = \"Au\"\nmeasurand = \"total-Au/all-phases\"\nbalance_basis = \"complete-two-product/no-inventory-change\"\nfeed = \"feed.toml\"\nconcentrate = \"concentrate.toml\"\ntailings = \"tailings.toml\"\n",
            utc(start + HOUR + NS),
        )).unwrap();
        result
    }

    fn command(&self, verb: &str, options: &[&str]) -> Command {
        let mut args = vec![
            "forecast".into(),
            "journal".into(),
            verb.into(),
            "--journal".into(),
            "journal".into(),
            "--head".into(),
            self.head.to_hex(),
        ];
        args.extend(options.iter().map(|value| value.to_string()));
        if !matches!(verb, "report" | "inspect" | "compare" | "verify-plant") {
            args.push("--trust-host-clock".into());
        }
        let (command, _, _) = fmine_cli::parse_args(&args, &self.root).unwrap();
        let CliCommand::ForecastJournal(command) = command else {
            panic!("forecast command parser route")
        };
        command
    }

    fn update_head(&mut self, result: &Envelope) {
        let payload = parse(result.payload_json.as_bytes()).unwrap();
        self.head = Digest32(parse_hex32(field(&payload, &["head"]).as_str().unwrap()).unwrap());
    }

    fn ok(&mut self, verb: &str, options: &[&str], at: i64) -> Json {
        let command = self.command(verb, options);
        let result = execute_with_clock(&command, &self.root, &Clock(at));
        assert_eq!(result.exit_code, 0, "{verb}: {}", result.to_json());
        self.update_head(&result);
        parse(result.payload_json.as_bytes()).unwrap()
    }

    fn freeze(&mut self, name: &str, members: &str, rule: &str, at: i64) {
        let path = format!("{name}.toml");
        fs::write(self.root.join(&path), format!(
            "[[cohort]]\nname = \"{name}\"\nmembers = \"{members}\"\nresolution_rule = \"{rule}\"\neligible_sources = \"synthetic:plant-balance\"\nalpha = \"0.2\"\n"
        )).unwrap();
        self.ok("freeze", &["--request", &path], at);
    }

    fn resolve(&self, cohort: &str, alias: &str) -> Command {
        self.command(
            "resolve-plant",
            &[
                "--cohort",
                cohort,
                "--forecast",
                alias,
                "--input",
                "observations/observation.toml",
            ],
        )
    }

    fn refuse(&self, command: &Command, clock: &dyn WallClock) {
        let before = fs::read(self.root.join("journal")).unwrap();
        let result = execute_with_clock(command, &self.root, clock);
        assert_ne!(
            result.exit_code,
            0,
            "unexpected admission: {}",
            result.to_json()
        );
        assert!(result.refusal.is_some());
        assert_eq!(fs::read(self.root.join("journal")).unwrap(), before);
        assert_eq!(
            Journal::open_read(&self.root.join("journal"), Some(self.head))
                .unwrap()
                .head(),
            self.head
        );
    }

    fn change(&self, path: &str, from: &str, to: &str) {
        let path = self.root.join(path);
        let original = fs::read_to_string(&path).unwrap();
        assert!(original.contains(from), "{from}");
        fs::write(path, original.replace(from, to)).unwrap();
    }
}

#[test]
fn raw_plant_recovery_is_exact_scored_and_replayed_without_current_sources_or_clock() {
    let mut fixture = Fixture::new("recovery");
    let before = fs::read(fixture.root.join("journal")).unwrap();
    let result = execute_with_clock(
        &fixture.resolve("monthly", "prediction"),
        &fixture.root,
        &Clock(fixture.start + HOUR + 3 * NS),
    );
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    fixture.update_head(&result);
    assert!(
        fs::read(fixture.root.join("journal"))
            .unwrap()
            .starts_with(&before)
    );
    let payload = parse(result.payload_json.as_bytes()).unwrap();
    assert_eq!(
        field(&payload, &["operation_version_counts", "v14"]).u64(),
        Some(1)
    );
    assert_eq!(
        field(&payload, &["operation_version_counts", "v16"]).u64(),
        Some(1)
    );
    let admissions = field(&payload, &["source_admissions"]).as_array().unwrap();
    assert_eq!(admissions.len(), 1);
    let evidence = field(&admissions[0], &["evidence"]);
    assert_eq!(
        field(evidence, &["basis"]).as_str(),
        Some("source_derived_unverified")
    );
    assert_eq!(
        field(evidence, &["observation", "exact_value", "numerator"]).as_str(),
        Some("4")
    );
    assert_eq!(
        field(evidence, &["observation", "exact_value", "denominator"]).as_str(),
        Some("5")
    );
    assert_eq!(
        field(evidence, &["observation", "feed_metal_kg", "denominator"]).as_str(),
        Some("100")
    );
    assert_eq!(
        field(
            evidence,
            &["observation", "concentrate_metal_kg", "denominator"]
        )
        .as_str(),
        Some("125")
    );
    assert_eq!(field(evidence, &["observation", "value"]).f64(), Some(0.8));
    for role in ["feed", "concentrate", "tailings"] {
        let original =
            fs::read_to_string(fixture.root.join(format!("observations/{role}.toml"))).unwrap();
        assert_eq!(
            field(
                evidence,
                &["observation", "sources", role, "submitted_worksheet"]
            )
            .as_str(),
            Some(original.as_str()),
        );
        assert_eq!(
            field(evidence, &["observation", "sources", role, "sha256"]).as_str(),
            Some(sha256(original.as_bytes()).to_hex().as_str()),
        );
    }
    let markdown = result.rendered_markdown.as_ref().unwrap();
    assert!(markdown.contains("Exact recovery: 4/5"));
    assert!(markdown.contains("source_derived_unverified"));
    assert!(markdown.contains("retained-observation-marker-feed"));
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);

    let cut = utc(fixture.start + HOUR + 4 * NS);
    let assessed = fixture.ok(
        "assess",
        &["--cohort", "monthly", "--as-of", &cut],
        fixture.start + HOUR + 6 * NS,
    );
    let report = field(&assessed, &["report"]);
    let summary = field(report, &["assessment", "summary"]);
    assert_eq!(field(summary, &["n_members"]).u64(), Some(1));
    assert_eq!(field(summary, &["n_resolved"]).u64(), Some(1));
    // E|X-.8| - E|X-X'|/2 for equal atoms .9 and .6 = .15 - .075 = 3/40.
    let loss = field(summary, &["crps", "observed_mean"]).f64().unwrap();
    assert!((loss - 3.0 / 40.0).abs() < 1e-12, "{loss}");
    let members = field(report, &["assessment", "members"])
        .as_array()
        .unwrap();
    assert_eq!(field(&members[0], &["source_admission"]), evidence);
    let digest = field(report, &["assessment", "digest"])
        .as_str()
        .unwrap()
        .to_string();
    for source in [
        "production.toml",
        "grinding.toml",
        "recovery.toml",
        "observations/observation.toml",
        "observations/feed.toml",
        "observations/concentrate.toml",
        "observations/tailings.toml",
    ] {
        fs::write(fixture.root.join(source), "changed after admission").unwrap();
    }
    let retained = fs::read(fixture.root.join("journal")).unwrap();
    let retrieved = execute_with_clock(
        &fixture.command("report", &["--assessment", &digest]),
        &fixture.root,
        &NoClock,
    );
    assert_eq!(retrieved.exit_code, 0, "{}", retrieved.to_json());
    let reopened = parse(retrieved.payload_json.as_bytes()).unwrap();
    assert_eq!(field(&reopened, &["report"]), report);
    assert_eq!(fs::read(fixture.root.join("journal")).unwrap(), retained);
}

#[test]
fn identical_physical_observation_keeps_identical_citations_for_two_frozen_models() {
    let mut fixture = Fixture::new("recovery");
    fixture.ok(
        "register-plant",
        &[
            "--input",
            "production.toml",
            "--grinding",
            "grinding.toml",
            "--recovery",
            "recovery.toml",
            "--id",
            "other-model",
            "--target",
            "recovery",
            "--author-role",
            "other-model-role",
            "--time-basis",
            "utc-unix-ns",
        ],
        fixture.start - 30 * NS,
    );
    fixture.freeze("other", "other-model", RULE, fixture.start - 20 * NS);
    for (cohort, alias, second) in [("monthly", "prediction", 3), ("other", "other-model", 4)] {
        let result = execute_with_clock(
            &fixture.resolve(cohort, alias),
            &fixture.root,
            &Clock(fixture.start + HOUR + second * NS),
        );
        assert_eq!(result.exit_code, 0, "{}", result.to_json());
        fixture.update_head(&result);
    }
    let cut = utc(fixture.start + HOUR + 5 * NS);
    let mut digests = Vec::new();
    for name in ["monthly", "other"] {
        let assessment = fixture.ok(
            "assess",
            &["--cohort", name, "--as-of", &cut],
            fixture.start + HOUR + 7 * NS,
        );
        digests.push(
            field(&assessment, &["report", "assessment", "digest"])
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let compared = execute_with_clock(
        &fixture.command(
            "compare",
            &[
                "--left",
                &digests[0],
                "--right",
                &digests[1],
                "--targets",
                "supplied",
                "--score",
                "crps",
            ],
        ),
        &fixture.root,
        &NoClock,
    );
    assert_eq!(compared.exit_code, 0, "{}", compared.to_json());
    let compared = parse(compared.payload_json.as_bytes()).unwrap();
    assert_eq!(field(&compared, &["comparison", "n_paired"]).u64(), Some(1));
    assert_eq!(
        field(&compared, &["comparison", "n_unmatched"]).u64(),
        Some(0)
    );
}

#[test]
fn plant_observation_admission_refuses_incompatible_or_incomplete_sources_atomically() {
    for (path, from, to) in [
        (
            "observations/observation.toml",
            "synthetic-ball-mill",
            "another-circuit",
        ),
        (
            "observations/observation.toml",
            "synthetic-shift-blend",
            "another-blend",
        ),
        (
            "observations/observation.toml",
            "invented-ore-domain",
            "another-domain",
        ),
        (
            "observations/observation.toml",
            "total-non-water-dry-matter/v1",
            "wet",
        ),
        (
            "observations/observation.toml",
            "synthetic:plant-balance",
            "ineligible-source",
        ),
        (
            "observations/observation.toml",
            "complete-two-product/no-inventory-change",
            "unknown-inventory",
        ),
        (
            "observations/observation.toml",
            "element = \"Au\"",
            "element = \"Cu\"",
        ),
        (
            "observations/feed.toml",
            "synthetic-plant/v1",
            "another-boundary",
        ),
        ("observations/feed.toml", "unit = \"kg\"", "unit = \"lb\""),
        (
            "observations/concentrate.toml",
            "value = \"100\"",
            "value = \"101\"",
        ),
        (
            "observations/concentrate.toml",
            "value = \"80\"",
            "value = \"79\"",
        ),
        (
            "observations/tailings.toml",
            "material = \"tailings\"",
            "material = \"feed\"",
        ),
        (
            "observations/tailings.toml",
            "element = \"Au\"",
            "element = \"Cu\"",
        ),
        (
            "observations/observation.toml",
            "feed = \"feed.toml\"",
            "feed = \"unavailable.toml\"",
        ),
    ] {
        let fixture = Fixture::new("recovery");
        fixture.change(path, from, to);
        fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);
    }
    for target in [
        "throughput",
        "dry-tonnes",
        "contained-metal",
        "recovered-metal",
    ] {
        let fixture = Fixture::new(target);
        fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);
    }
    let fixture = Fixture::with_dry_basis("recovery", "another-declared-dry-basis");
    fixture.change(
        "observations/observation.toml",
        "total-non-water-dry-matter/v1",
        "another-declared-dry-basis",
    );
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);
    let mut fixture = Fixture::new("recovery");
    fixture.freeze(
        "wrong-rule",
        "prediction",
        "arbitrary-supplied-rule",
        fixture.start - 30 * NS,
    );
    fixture.refuse(&fixture.resolve("wrong-rule", "prediction"), &NoClock);
    fixture.refuse(&fixture.resolve("absent", "prediction"), &NoClock);
    fixture.change(
        "observations/feed.toml",
        &format!("start_ns = \"{}\"", fixture.start),
        &format!("start_ns = \"{}\"", fixture.start + NS),
    );
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);

    let fixture = Fixture::new("recovery");
    for (role, dry) in [
        ("feed", "1000"),
        ("concentrate", "100"),
        ("tailings", "900"),
    ] {
        fs::write(
            fixture.root.join(format!("observations/{role}.toml")),
            material(role, dry, "0", fixture.start),
        )
        .unwrap();
    }
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);

    let fixture = Fixture::new("recovery");
    // Each source remains below 64 KiB; the combined operation must still refuse without truncation.
    for role in ["feed", "concentrate", "tailings"] {
        let path = fixture.root.join(format!("observations/{role}.toml"));
        let text = fs::read_to_string(&path).unwrap();
        fs::write(path, format!("{text}\n#{}\n", "padding".repeat(4500))).unwrap();
    }
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);
}

#[test]
fn observation_availability_respects_physical_end_and_the_earliest_writer_clock() {
    let fixture = Fixture::new("recovery");
    fixture.refuse(
        &fixture.resolve("monthly", "prediction"),
        &Clock(fixture.start + HOUR + NS), // lower bound is physical end, before declared availability.
    );
    fixture.change(
        "observations/observation.toml",
        &utc(fixture.start + HOUR + NS),
        &utc(fixture.start + HOUR),
    );
    fixture.refuse(&fixture.resolve("monthly", "prediction"), &NoClock);
}

fn rewrite_history(root: &Path, name: &str, mut records: Vec<Record>) -> Digest32 {
    let mut previous = Digest32::default();
    let mut bytes = Vec::new();
    for record in &mut records {
        record.previous = previous;
        bytes.extend(record.to_line().unwrap());
        previous = record.chain_head().unwrap();
    }
    fs::write(root.join(name), bytes).unwrap();
    previous
}

#[test]
fn recomputation_rejects_semantic_tampering_even_with_a_valid_rebuilt_journal_chain() {
    let mut fixture = Fixture::new("recovery");
    let result = execute_with_clock(
        &fixture.resolve("monthly", "prediction"),
        &fixture.root,
        &Clock(fixture.start + HOUR + 3 * NS),
    );
    assert_eq!(result.exit_code, 0, "{}", result.to_json());
    fixture.update_head(&result);
    let records = Journal::open_read(&fixture.root.join("journal"), Some(fixture.head))
        .unwrap()
        .records()
        .to_vec();
    let source = fs::read_to_string(fixture.root.join("observations/feed.toml")).unwrap();
    let changed = source.replace(
        "retained-observation-marker-feed",
        "mutatedx-observation-marker-feed",
    );
    assert_eq!(source.len(), changed.len());
    for (i, mutation) in [
        "version2",
        "version14",
        "legacy",
        "wrong-table",
        "scalar",
        "forecast-id",
        "cohort-digest",
        "source",
        "source-and-hash",
    ]
    .into_iter()
    .enumerate()
    {
        let mut copy = records.clone();
        let Entry::Operation(operation) = &mut copy.last_mut().unwrap().entry else {
            panic!("source-derived outcome operation")
        };
        *operation = match mutation {
            "version2" => operation.replace(
                "fmine.forecast-operation.v16",
                "fmine.forecast-operation.v2",
            ),
            "version14" => operation.replace(
                "fmine.forecast-operation.v16",
                "fmine.forecast-operation.v14",
            ),
            "legacy" => operation.replacen("schema = \"fmine.forecast-operation.v16\"\n", "", 1),
            "wrong-table" => operation.replace("[[plant_outcome]]", "[[outcome]]"),
            "scalar" => operation.replace(
                &format!("value_bits = \"{:016x}\"", 0.8_f64.to_bits()),
                &format!("value_bits = \"{:016x}\"", 0.9_f64.to_bits()),
            ),
            "forecast-id" | "cohort-digest" => {
                let key = if mutation == "forecast-id" {
                    "forecast_id"
                } else {
                    "cohort_digest"
                };
                let line = operation
                    .lines()
                    .find(|line| line.starts_with(&format!("{key} = ")))
                    .unwrap();
                operation.replacen(
                    line,
                    &format!("{key} = \"{}\"", Digest32::default().to_hex()),
                    1,
                )
            }
            "source" => operation.replace(
                "retained-observation-marker-feed",
                "mutatedx-observation-marker-feed",
            ),
            "source-and-hash" => operation
                .replace(
                    "retained-observation-marker-feed",
                    "mutatedx-observation-marker-feed",
                )
                .replacen(
                    &sha256(source.as_bytes()).to_hex(),
                    &sha256(changed.as_bytes()).to_hex(),
                    1,
                ),
            _ => unreachable!(),
        };
        let name = format!("tampered-{i}");
        let head = rewrite_history(&fixture.root, &name, copy);
        let before = fs::read(fixture.root.join(&name)).unwrap();
        let result = execute_with_clock(
            &Command::Inspect {
                journal: name.clone().into(),
                head: Some(head),
            },
            &fixture.root,
            &NoClock,
        );
        assert_ne!(result.exit_code, 0, "{mutation}: {}", result.to_json());
        assert_eq!(fs::read(fixture.root.join(name)).unwrap(), before);
    }
}
