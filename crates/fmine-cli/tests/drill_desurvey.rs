//! SYNTHETIC source-to-Corebox command controls (BD-188, bd-zx57).
//! Determinism: DeterministicPerIsa (minimum-curvature coordinates and covariance).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_cli::{Envelope, OutputFormat, invoke};
use fmine_core::{Digest32, hash::sha256};
use fmine_io::omf::json::{Json, parse};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
use fmine_object::store::CasStore;

const MANIFEST: &str = include_str!("../../../tests/drill/acquire_synthetic.toml");
const COLLARS: &str = include_str!("../../../tests/drill/collars.csv");
const SURVEYS: &str = include_str!("../../../tests/drill/surveys.csv");
const ASSAYS: &str = include_str!("../../../tests/drill/assays.csv");
const DIALECTS: &str = include_str!("../../../registries/io_dialects.toml");
const INPUT: &str = "inputs/drill/acquire_synthetic.toml";
const CAS: &str = "retained-source-cas";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-drill-desurvey-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(root.join("inputs/drill")).expect("input directory");
    std::fs::create_dir_all(root.join("registries")).expect("registry directory");
    for (path, contents) in [
        (INPUT, MANIFEST),
        ("inputs/drill/collars.csv", COLLARS),
        ("inputs/drill/surveys.csv", SURVEYS),
        ("inputs/drill/assays.csv", ASSAYS),
        ("registries/io_dialects.toml", DIALECTS),
        (
            "registries/units.toml",
            include_str!("../../../registries/units.toml"),
        ),
        (
            "registries/crs.toml",
            include_str!("../../../registries/crs.toml"),
        ),
    ] {
        std::fs::write(root.join(path), contents).expect("synthetic source");
    }
    std::fs::canonicalize(root).expect("canonical fixture root")
}

fn arguments() -> Vec<String> {
    ["drill", "desurvey", "--input", INPUT, "--cas", CAS]
        .map(str::to_owned)
        .into()
}

fn execute(root: &Path) -> Envelope {
    let (envelope, format) = invoke(&arguments(), root);
    assert_eq!(format, OutputFormat::Json);
    envelope
}

fn successful(root: &Path) -> Json {
    let envelope = execute(root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    parse(envelope.to_json().as_bytes()).expect("valid JSON envelope")
}

fn field<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter()
        .fold(value, |value, key| value.get(key).expect(key))
}

fn report(value: &Json) -> &Json {
    field(value, &["payload", "report"])
}

fn hole(value: &Json) -> &Json {
    &field(report(value), &["geometry", "holes"])
        .as_array()
        .expect("holes")[0]
}

fn identity(value: &Json) -> (&str, &str) {
    (
        field(value, &["payload", "receipt_sha256"])
            .as_str()
            .expect("receipt"),
        field(hole(value), &["claim", "id"])
            .as_str()
            .expect("Claim ID"),
    )
}

fn flags_are_false(value: &Json) {
    let flags = field(value, &["flags"]).as_object().expect("flags");
    assert_eq!(flags.len(), 4);
    for key in [
        "reserve_disclosure_claim",
        "execution_authorized",
        "geotechnical_signoff",
        "regulatory_compliance_claim",
    ] {
        assert_eq!(
            field(value, &["flags", key]).as_bool(),
            Some(false),
            "{key}"
        );
    }
    assert_eq!(field(value, &["anchor_id"]), &Json::Null);
    assert_eq!(field(value, &["certificate"]), &Json::Null);
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

fn position(value: &Json, depth_mm: u64, expected_east_variance: f64) {
    assert_eq!(field(value, &["depth_mm"]).u64(), Some(depth_mm));
    let point = field(value, &["point_m"]).as_array().expect("point");
    for (actual, expected) in
        point
            .iter()
            .zip([500_000.0, 4_500_000.0 + depth_mm as f64 / 1000.0, 1500.0])
    {
        near(actual.f64().expect("coordinate"), expected);
    }
    let direction = field(value, &["direction_enu"])
        .as_array()
        .expect("direction");
    for (actual, expected) in direction.iter().zip([0.0, 1.0, 0.0]) {
        near(actual.f64().expect("direction component"), expected);
    }
    let covariance = field(value, &["covariance_m2"])
        .as_array()
        .expect("covariance");
    let collar = field(value, &["collar_covariance_m2"])
        .as_array()
        .expect("collar covariance");
    let terms = field(value, &["covariance_terms_m2"])
        .as_array()
        .expect("term covariances");
    assert_eq!(covariance.len(), 3);
    assert_eq!(collar.len(), 3);
    assert_eq!(terms.len(), 2, "separate dip and azimuth terms");
    for (i, row) in covariance.iter().enumerate() {
        let row = row.as_array().expect("covariance row");
        assert_eq!(row.len(), 3);
        for (j, actual) in row.iter().enumerate() {
            let expected = if i == j {
                [expected_east_variance, 0.04, 0.09][i]
            } else {
                0.0
            };
            let actual = actual.f64().expect("covariance entry");
            assert!(
                (actual - expected).abs() < 1e-8,
                "covariance {actual} != {expected}"
            );
            let collar_part = collar[i].as_array().expect("collar row")[j]
                .f64()
                .expect("collar entry");
            near(
                collar_part,
                if i == j { [0.04, 0.04, 0.09][i] } else { 0.0 },
            );
            let sum = terms.iter().fold(collar_part, |sum, term| {
                sum + field(term, &["covariance_m2"])
                    .as_array()
                    .expect("term matrix")[i]
                    .as_array()
                    .expect("term row")[j]
                    .f64()
                    .expect("term entry")
            });
            near(actual, sum);
        }
    }
    assert_eq!(field(value, &["extrapolated"]).as_bool(), Some(false));
}

fn digest(hex: &str) -> Digest32 {
    assert_eq!(hex.len(), 64);
    let mut bytes = [0; 32];
    for (target, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
        *target = u8::from_str_radix(std::str::from_utf8(pair).expect("hex UTF-8"), 16)
            .expect("hex digest");
    }
    Digest32(bytes)
}

fn sources_and_cas_match(root: &Path, source_report: &Json) {
    let sources = field(source_report, &["sources"])
        .as_array()
        .expect("sources");
    assert_eq!(
        sources.len(),
        7,
        "manifest, three exports and three interpretation registries"
    );
    for source in sources {
        let path = field(source, &["path"]).as_str().expect("source path");
        let bytes = std::fs::read(path).expect("anchored source bytes");
        assert_eq!(
            field(source, &["sha256"]).as_str(),
            Some(sha256(&bytes).to_string().as_str())
        );
    }
    let store = CasStore::open(root.join(CAS)).expect("explicit CAS");
    let tables = field(source_report, &["tables"])
        .as_array()
        .expect("tables");
    assert_eq!(tables.len(), 3);
    for table in tables {
        let path = field(table, &["path"]).as_str().expect("table path");
        let original = std::fs::read(path).expect("original export");
        let root = digest(field(table, &["cas_root"]).as_str().expect("CAS root"));
        assert_eq!(
            store.read_object(&root).expect("retained original bytes"),
            original
        );
        assert_eq!(
            field(table, &["sha256"]).as_str(),
            Some(sha256(&original).to_string().as_str())
        );
        assert_eq!(field(table, &["bytes"]).u64(), Some(original.len() as u64));
    }
}

#[test]
fn native_command_emits_known_geometry_shared_station_covariance_and_exact_censored_assays() {
    let root = fixture();
    let envelope = execute(&root);
    assert_eq!(envelope.exit_code, 0, "{}", envelope.to_json());
    let output = || {
        Command::new(env!("CARGO_BIN_EXE_fmine"))
            .current_dir(&root)
            .args(arguments())
            .output()
            .expect("native fmine")
    };
    let first = output();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    assert!(first.stderr.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&first.stdout).trim_end(),
        envelope.to_json()
    );
    assert_eq!(
        first.stdout,
        output().stdout,
        "same inputs reproduce identical output bytes"
    );
    let json = parse(&first.stdout).expect("native JSON envelope");
    flags_are_false(&json);
    assert_eq!(field(&json, &["verb"]).as_str(), Some("drill desurvey"));
    assert!(
        field(&json, &["no_claim"])
            .as_str()
            .expect("scope")
            .contains("uncalibrated")
    );
    let source_report = report(&json);
    sources_and_cas_match(&root, source_report);
    assert_eq!(
        field(source_report, &["rejected_rows"])
            .as_array()
            .expect("rejections")
            .len(),
        0
    );
    assert_eq!(
        field(source_report, &["domain_admitted", "surveys"]).u64(),
        Some(3)
    );
    assert_eq!(
        field(source_report, &["geometry", "coordinate_order"]),
        &Json::Array(
            ["east", "north", "up"]
                .map(|s| Json::String(s.into()))
                .into()
        )
    );
    assert_eq!(
        field(source_report, &["geometry", "covariance_unit"]).as_str(),
        Some("m2")
    );
    assert_eq!(
        field(source_report, &["geometry", "error_model"]).as_str(),
        Some("random")
    );
    let terms = field(source_report, &["geometry", "error_terms"])
        .as_array()
        .expect("declared error terms");
    assert_eq!(terms.len(), 2);
    for (term, code, angle) in [(&terms[0], "DIP-R", "dip"), (&terms[1], "AZ-R", "azimuth")] {
        assert_eq!(field(term, &["code"]).as_str(), Some(code));
        assert_eq!(field(term, &["angle"]).as_str(), Some(angle));
        assert_eq!(field(term, &["propagation"]).as_str(), Some("random"));
        assert_eq!(field(term, &["variance_share"]).f64(), Some(1.0));
    }
    assert_eq!(
        field(source_report, &["geometry", "horizontal_sigma_basis"]).as_str(),
        Some("per_grid_axis")
    );
    let claim = field(hole(&json), &["claim"]);
    for (key, expected) in [
        ("metric", "drill.desurvey/1"),
        ("evidence", "Assumed"),
        ("state", "Estimated"),
        ("confidentiality", "Internal"),
        ("calibration", "uncalibrated"),
    ] {
        assert_eq!(field(claim, &[key]).as_str(), Some(expected));
    }
    assert_eq!(
        field(claim, &["sources"]),
        field(source_report, &["sources"])
    );
    let result = field(hole(&json), &["result"]);
    assert_eq!(field(result, &["collar_source_row"]).u64(), Some(2));
    assert_eq!(field(result, &["max_depth_mm"]).u64(), Some(200_000));
    position(field(result, &["collar"]), 0, 0.04);
    let angular_variance = (std::f64::consts::PI / 180.0).powi(2);
    // Sensitivities at 200 m are 50, 100, 50 m/rad: the middle survey is shared by both arcs.
    position(
        field(result, &["bottom"]),
        200_000,
        0.04 + 15_000.0 * angular_variance,
    );
    let stations = field(result, &["stations"]).as_array().expect("stations");
    assert_eq!(stations.len(), 3);
    for (i, station) in stations.iter().enumerate() {
        assert_eq!(field(station, &["source_row"]).u64(), Some(i as u64 + 2));
        assert_eq!(field(station, &["suspect"]).as_bool(), Some(false));
    }
    let assays = field(result, &["assays"]).as_array().expect("assays");
    assert_eq!(assays.len(), 2);
    assert_eq!(field(&assays[0], &["source_row"]).u64(), Some(2));
    assert_eq!(field(&assays[0], &["sample_id"]).as_str(), Some("SYN-A"));
    assert_eq!(field(&assays[0], &["from_m_exact"]).as_str(), Some("100"));
    assert_eq!(field(&assays[0], &["to_m_exact"]).as_str(), Some("150"));
    // At 150 m, 50, 87.5, 12.5 m/rad sensitivities include the partial second arc.
    position(
        field(&assays[0], &["end"]),
        150_000,
        0.04 + 10_312.5 * angular_variance,
    );
    assert_eq!(field(&assays[0], &["end"]), field(&assays[1], &["start"]));
    for (key, expected) in [
        ("state", "below_detection"),
        ("limit", "1/200"),
        ("raw", "<0.005"),
    ] {
        assert_eq!(
            field(&assays[0], &["readings", "AU_GPT", key]).as_str(),
            Some(expected)
        );
    }
    assert_eq!(
        field(&assays[0], &["readings", "CU_PCT", "state"]).as_str(),
        Some("placeholder")
    );
    assert_eq!(
        field(&assays[0], &["readings", "CU_PCT", "raw"]).as_str(),
        Some("NSS")
    );
    assert_eq!(
        field(&assays[1], &["readings", "AU_GPT", "value"]).as_str(),
        Some("5/4")
    );
    assert_eq!(
        field(&assays[1], &["readings", "CU_PCT", "value"]).as_str(),
        Some("3/10")
    );
    let body = envelope
        .payload_json
        .split_once(",\"report\":")
        .expect("report bytes")
        .1;
    let body = body.strip_suffix('}').expect("payload close");
    assert_eq!(identity(&json).0, sha256(body.as_bytes()).to_string());
}

#[test]
fn named_xlsx_sheets_match_csv_geometry_and_retain_original_workbook_bytes() {
    // These six-column synthetic CSV fixtures contain no quoted or multiline cells.
    fn sheet(name: &str, csv: &str) -> MinimalSheet {
        let mut cells = Vec::new();
        for (row, line) in csv.lines().enumerate() {
            for (column, value) in line.split(',').enumerate() {
                let reference = format!("{}{}", ["A", "B", "C", "D", "E", "F"][column], row + 1);
                let cell = value
                    .parse::<f64>()
                    .map_or_else(|_| MinimalCell::Text(value.into()), MinimalCell::Number);
                cells.push((reference, cell));
            }
        }
        MinimalSheet {
            name: name.into(),
            hidden: false,
            cells,
        }
    }

    let root = fixture();
    let csv = successful(&root);
    let workbook = build_minimal_xlsx(&[
        sheet("Collars", COLLARS),
        sheet("Surveys", SURVEYS),
        sheet("Assays", ASSAYS),
    ]);
    std::fs::write(root.join("inputs/drill/export.xlsx"), &workbook).expect("synthetic workbook");
    let mut manifest = MANIFEST.to_owned();
    for (filename, sheet) in [
        ("collars", "Collars"),
        ("surveys", "Surveys"),
        ("assays", "Assays"),
    ] {
        manifest = manifest.replace(
            &format!("path = \"{filename}.csv\"\nformat = \"csv\""),
            &format!("path = \"export.xlsx\"\nformat = \"xlsx\"\nsheet = \"{sheet}\""),
        );
    }
    std::fs::write(root.join(INPUT), manifest).expect("named workbook declarations");
    let xlsx = successful(&root);
    flags_are_false(&xlsx);
    sources_and_cas_match(&root, report(&xlsx));
    for key in ["collar", "bottom", "assays"] {
        assert_eq!(
            field(hole(&csv), &["result", key]),
            field(hole(&xlsx), &["result", key]),
            "CSV/XLSX {key} parity includes exact censored readings and covariance"
        );
    }
    assert_ne!(
        identity(&csv),
        identity(&xlsx),
        "different source bytes have different identities"
    );
    let store = CasStore::open(root.join(CAS)).expect("explicit source CAS");
    for table in field(report(&xlsx), &["tables"])
        .as_array()
        .expect("tables")
    {
        let expected_sheet = match field(table, &["role"]).as_str().expect("role") {
            "collar" => "Collars",
            "survey" => "Surveys",
            "assay" => "Assays",
            other => panic!("unexpected table role {other}"),
        };
        assert_eq!(field(table, &["format"]).as_str(), Some("xlsx"));
        assert_eq!(field(table, &["sheet"]).as_str(), Some(expected_sheet));
        assert_eq!(
            field(table, &["sha256"]).as_str(),
            Some(sha256(&workbook).to_string().as_str())
        );
        let cas_root = digest(field(table, &["cas_root"]).as_str().expect("CAS root"));
        assert_eq!(
            store.read_object(&cas_root).expect("original workbook"),
            workbook
        );
    }
}

#[test]
fn coordinates_sigma_declarations_and_registry_bytes_bind_both_claim_and_receipt() {
    let root = fixture();
    let initial = successful(&root);
    for (mutation, (path, changed)) in [
        (
            "inputs/drill/collars.csv",
            COLLARS.replace("500000", "500001"),
        ),
        (
            INPUT,
            MANIFEST.replace("sigma_azimuth_deg = \"1\"", "sigma_azimuth_deg = \"2\""),
        ),
        (INPUT, MANIFEST.replace("no depth, CRS", "no depth or CRS")),
        (
            "registries/io_dialects.toml",
            format!("{DIALECTS}\n# Synthetic interpretation snapshot revision.\n"),
        ),
        (
            INPUT,
            MANIFEST.replace(
                "survey_error_model = \"random\"",
                "survey_error_model = \"systematic\"",
            ),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        std::fs::write(root.join(path), changed).expect("mutated source");
        let changed = successful(&root);
        assert_ne!(
            identity(&initial).0,
            identity(&changed).0,
            "receipt must bind {path}"
        );
        assert_ne!(
            identity(&initial).1,
            identity(&changed).1,
            "Claim must bind {path}"
        );
        if path.ends_with("collars.csv") {
            assert_eq!(
                field(hole(&changed), &["result", "bottom", "point_m"])
                    .as_array()
                    .expect("point")[0]
                    .f64(),
                Some(500_001.0)
            );
        }
        if mutation == 1 {
            position(
                field(hole(&changed), &["result", "bottom"]),
                200_000,
                0.04 + 60_000.0 * (std::f64::consts::PI / 180.0).powi(2),
            );
        } else if mutation == 4 {
            assert_eq!(
                field(report(&changed), &["geometry", "error_model"]).as_str(),
                Some("systematic")
            );
            let angular_variance = (std::f64::consts::PI / 180.0).powi(2);
            position(
                field(hole(&changed), &["result", "bottom"]),
                200_000,
                0.04 + 40_000.0 * angular_variance,
            );
            let assays = field(hole(&changed), &["result", "assays"])
                .as_array()
                .expect("assays");
            position(
                field(&assays[0], &["end"]),
                150_000,
                0.04 + 22_500.0 * angular_variance,
            );
        } else if mutation > 1 {
            assert_eq!(
                field(hole(&initial), &["result"]),
                field(hole(&changed), &["result"]),
                "source declarations bind identity even when numeric geometry is unchanged"
            );
        }
        std::fs::write(root.join("inputs/drill/collars.csv"), COLLARS).expect("restore collar");
        std::fs::write(root.join(INPUT), MANIFEST).expect("restore manifest");
        std::fs::write(root.join("registries/io_dialects.toml"), DIALECTS)
            .expect("restore dialect");
    }
    assert_eq!(
        identity(&initial),
        identity(&successful(&root)),
        "restored input snapshot restores identity"
    );
}

#[test]
fn a_quarantined_survey_keeps_the_import_report_but_refuses_the_entire_trace() {
    let root = fixture();
    std::fs::write(
        root.join("inputs/drill/surveys.csv"),
        SURVEYS.replace("SYN-01,100,0,0,GYRO", "SYN-01,100,0,100,GYRO"),
    )
    .expect("out-of-domain middle survey");
    let envelope = execute(&root);
    assert_eq!(envelope.status, "refused", "{}", envelope.to_json());
    assert_ne!(envelope.exit_code, 0);
    let json = parse(envelope.to_json().as_bytes()).expect("refusal JSON");
    flags_are_false(&json);
    let source_report = field(&json, &["payload"]);
    assert_eq!(field(source_report, &["geometry"]), &Json::Null);
    assert_eq!(
        field(source_report, &["domain_admitted", "surveys"]).u64(),
        Some(2)
    );
    let rejections = field(source_report, &["rejected_rows"])
        .as_array()
        .expect("rejections");
    assert_eq!(rejections.len(), 1);
    assert_eq!(field(&rejections[0], &["table"]).as_str(), Some("survey"));
    assert_eq!(field(&rejections[0], &["row"]).u64(), Some(3));
    sources_and_cas_match(&root, source_report);
    assert!(!envelope.payload_json.contains("\"holes\":"));
}

#[test]
fn unsupported_frames_inexact_depths_and_missing_accuracy_never_fabricate_geometry() {
    for (path, changed) in [
        (INPUT, MANIFEST.replace("EPSG:32611", "EPSG:4326")),
        (
            INPUT,
            MANIFEST.replace(
                "azimuth_reference = \"grid\"",
                "azimuth_reference = \"magnetic\"",
            ),
        ),
        (INPUT, MANIFEST.replace("sigma_azimuth_deg = \"1\"\n", "")),
        (
            INPUT,
            MANIFEST.replace("survey_error_model = \"random\"\n", ""),
        ),
        (
            INPUT,
            MANIFEST.replace(
                "survey_error_model = \"random\"",
                "survey_error_model = \"automatic\"",
            ),
        ),
        (
            INPUT,
            MANIFEST.replace("sigma_vertical_m = \"0.3\"", "sigma_vertical_m = \"NaN\""),
        ),
        (
            "inputs/drill/surveys.csv",
            SURVEYS.replace("SYN-01,100,", "SYN-01,100.0001,"),
        ),
        (
            "registries/io_dialects.toml",
            DIALECTS.replace(
                "column = \"EAST\"\nunit = \"m\"",
                "column = \"EAST\"\nunit = \"t\"",
            ),
        ),
    ] {
        let root = fixture();
        std::fs::write(root.join(path), changed).expect("invalid geometry declaration");
        let envelope = execute(&root);
        assert_eq!(envelope.status, "refused", "{path}: {}", envelope.to_json());
        assert_ne!(envelope.exit_code, 0);
        assert!(!envelope.payload_json.contains("\"holes\":"), "{path}");
        flags_are_false(&parse(envelope.to_json().as_bytes()).expect("refusal JSON"));
    }
}

#[test]
fn exports_resolve_relative_to_manifest_and_cas_requires_an_explicit_destination() {
    let root = fixture();
    for name in ["collars.csv", "surveys.csv", "assays.csv"] {
        std::fs::write(
            root.join(name),
            "invalid decoy at process working directory\n",
        )
        .expect("decoy");
    }
    let json = successful(&root);
    sources_and_cas_match(&root, report(&json));
    assert!(root.join(CAS).join("manifests").is_dir());
    assert!(!root.join("inputs/drill").join(CAS).exists());
    let args: Vec<String> = ["drill", "desurvey", "--input", INPUT]
        .map(str::to_owned)
        .into();
    let (envelope, _) = invoke(&args, &root);
    assert_eq!(envelope.exit_code, 2);
    assert_eq!(envelope.status, "usage_error");
    assert!(envelope.payload_json.contains("--cas"));
}
