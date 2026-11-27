//! DESIGN (BD-188, bd-zx57): source-bound acQuire exports become usable Corebox geometry.
//! Determinism: DeterministicPerIsa (exact input snapshots, minimum-curvature binary64 projection).

mod composite;
mod manifest;
mod render;

pub use composite::{
    ROBOT_DOCS_JSON as COMPOSITE_ROBOT_DOCS_JSON, USAGE as COMPOSITE_USAGE, VERB as COMPOSITE_VERB,
};

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::Refusal;
use fmine_io::acquire::{
    AcquireBundleInput, AcquireTableInput, drill::prepare_geometry, import_acquire_export_bundle,
};
use fmine_io::dialects::IoDialectRegistry;
use fmine_io::quarantine::MAX_QUARANTINE_BYTES;
use fmine_object::store::CasStore;
use fmine_registry::toml_subset::{TomlValue, parse_subset};

use crate::{Envelope, NextAction, journal_arguments, worksheet_input};
use manifest::invalid;

pub const VERB: &str = "drill desurvey";
pub const USAGE: &str = "drill desurvey --input <manifest.toml> --cas <directory>";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_OUTPUT_POSITIONS: usize = 20_000;
pub const NO_CLAIM: &str = "Geometry is conditional on supplied coordinates, grid bearings and one-sigma error declarations. Covariance is first-order and uncalibrated under the explicitly selected random or systematic angle-error model; depth error, CRS error, inter-hole correlation and undeclared instrument effects are excluded. Original export bytes are retained in the explicitly selected CAS. No assay QA/QC, custody chain, authority-ledger commit, qualified ISCWSA instrument model, reserve disclosure, execution authorization, geotechnical sign-off or regulatory compliance is established.";
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.drill-desurvey.v1","#,
    r#""command":"drill desurvey --input <manifest.toml> --cas <directory>","#,
    r#""input":"Quoted-string TOML, at most 1 MiB; file paths are relative to the manifest; table bytes at most 32 MiB each","#,
    r#""top_fields":["schema","crs","evidence_lane","survey_error_model","omitted_uncertainty"],"#,
    r#""evidence_lane":"synthetic or internal; all geometry Claims are Assumed, Internal and uncalibrated","#,
    r#""table_fields":["role","path","format","sheet"],"#,
    r#""tables":"Exactly collar, survey and assay; format csv or xlsx; xlsx requires a sheet, csv forbids one","#,
    r#""hole_fields":["hole_id","program","hole_type","collar_from","position_method","sigma_horizontal_m","sigma_vertical_m","instrument","run","sigma_azimuth_deg","sigma_dip_deg","magnetic_interference","azimuth_reference","survey_type_label"],"#,
    r#""hole_types":["diamond","rc","aircore","rab","sonic","blasthole"],"#,
    r#""position_methods":["design","dgnss","total_station","handheld_gnss","unknown"],"#,
    r#""instruments":["gyro","magnetic_multishot","magnetic_single_shot"],"#,
    r#""declarations":"One per collar; actual/planned collar, in/out survey run, grid azimuth reference and explicit quoted true/false interference; survey_type_label is optional and explicitly maps a printed vendor label","#,
    r#""uncertainty":"Required survey_error_model: random gives independent station angle errors; systematic shares each angle error across stations of one instrument and run. Collar axes are independent, with horizontal sigma applied separately to east and north. All stated angle variance is used exactly once.","#,
    r#""output":"Source SHA-256 and export CAS roots, exact assay readings and censoring, row-specific refusals, per-hole Claim, stations and assay endpoints with total and per-term east/north/up covariance in m2, extrapolation and suspect-station flags","#,
    r#""admission":"Any quarantined row refuses the whole geometry projection; source report is retained; no silent station removal or depth rounding; registered WGS84 UTM metre CRS only","#,
    r#""limits":"4096 holes; 20000 emitted positions including stations, assay endpoints, collars and hole bottoms; split larger inputs on complete holes","#,
    r#""example":"tests/drill/acquire_synthetic.toml","#,
    r#""side_effect":"Only the explicitly selected content-addressed store receives original export bytes; no authority records are written"}"#,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
    pub cas: PathBuf,
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    parse_for(args, "desurvey", USAGE)
}

pub fn parse_composite(args: &[String]) -> Result<Command, String> {
    parse_for(args, "composite", COMPOSITE_USAGE)
}

fn parse_for(args: &[String], subcommand: &str, usage: &str) -> Result<Command, String> {
    let (mut options, words) = journal_arguments(args, &[])?;
    let input = options.remove("--input").filter(|s| !s.trim().is_empty());
    let cas = options.remove("--cas").filter(|s| !s.trim().is_empty());
    if words != ["drill", subcommand] || !options.is_empty() {
        return Err(usage.into());
    }
    Ok(Command {
        input: input.ok_or(usage)?.into(),
        cas: cas.ok_or(usage)?.into(),
    })
}

struct SourceTable {
    role: String,
    bytes: Vec<u8>,
    source: SourceAnchor,
    format: String,
    sheet: Option<String>,
}

impl SourceTable {
    fn input(&self) -> AcquireTableInput<'_> {
        if self.format == "csv" {
            AcquireTableInput::Csv(&self.bytes)
        } else {
            AcquireTableInput::Xlsx {
                bytes: &self.bytes,
                sheet: self.sheet.as_deref(),
            }
        }
    }
}

// Only registered code membership is consumed here. Read it from the exact captured registry bytes,
// alongside the dialect parser, so a later on-disk edit cannot change an already-hashed interpretation.
fn codes(text: &str) -> Result<BTreeSet<String>, Refusal> {
    let mut result = BTreeSet::new();
    for line in text.lines() {
        if line
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "code")
        {
            let doc = parse_subset(line).map_err(|e| invalid(e.to_string()))?;
            let Some(TomlValue::Str(code)) = doc.top_get("code").map(|f| &f.value) else {
                return Err(invalid("registry code must be a quoted string"));
            };
            if code.is_empty() || !result.insert(code.clone()) {
                return Err(invalid("registry codes must be nonempty and unique"));
            }
        }
    }
    if result.is_empty() {
        return Err(invalid("registry contains no codes"));
    }
    Ok(result)
}

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect the drilling source and error-declaration contract",
    )]
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    execute_mode(command, root, false)
}

pub fn execute_composite(command: &Command, root: &Path) -> Envelope {
    execute_mode(command, root, true)
}

fn execute_mode(command: &Command, root: &Path, compositing: bool) -> Envelope {
    let (verb, key, title, no_claim) = if compositing {
        (
            COMPOSITE_VERB,
            "compositing",
            "Drill composite",
            composite::NO_CLAIM,
        )
    } else {
        (VERB, "geometry", "Drill desurvey", NO_CLAIM)
    };
    let evidence_description = if compositing {
        "Claims are Assumed, Internal and uncalibrated."
    } else {
        "Geometry is Assumed, Internal and uncalibrated."
    };
    let run = || -> Result<Envelope, Refusal> {
        let path = root.join(&command.input);
        let source_label = if compositing {
            "drill composite manifest"
        } else {
            "drill desurvey manifest"
        };
        let (text, source) = worksheet_input::read(&path, source_label, MAX_MANIFEST_BYTES)?;
        let declaration = manifest::parse(&text, compositing)?;
        let parent = path
            .parent()
            .ok_or_else(|| invalid("manifest has no parent directory"))?;
        let mut anchors = vec![source];
        let mut tables = Vec::new();
        for table in &declaration.tables {
            let (bytes, source) = worksheet_input::read_bytes(
                &parent.join(&table.path),
                "acQuire export",
                MAX_QUARANTINE_BYTES as u64,
            )?;
            anchors.push(source.clone());
            tables.push(SourceTable {
                role: table.role.clone(),
                bytes,
                source,
                format: table.format.clone(),
                sheet: table.sheet.clone(),
            });
        }
        let mut registries = Vec::new();
        for name in ["io_dialects.toml", "units.toml", "crs.toml"] {
            let (text, source) = worksheet_input::read(
                &root.join("registries").join(name),
                "drill interpretation registry",
                MAX_MANIFEST_BYTES,
            )?;
            anchors.push(source);
            registries.push(text);
        }
        let dialects =
            IoDialectRegistry::parse(&registries[0]).map_err(|e| invalid(e.to_string()))?;
        let units = codes(&registries[1])?;
        let crs = codes(&registries[2])?;
        let table = |role| {
            tables
                .iter()
                .find(|t| t.role == role)
                .ok_or_else(|| invalid(format!("missing {role} table")))
        };
        let input = AcquireBundleInput {
            collars: table("collar")?.input(),
            surveys: table("survey")?.input(),
            assays: table("assay")?.input(),
            site_crs: Some(&declaration.crs),
        };
        let store = CasStore::open(root.join(&command.cas)).map_err(|e| e.to_refusal())?;
        let bundle = import_acquire_export_bundle(&store, &dialects, &input, &units, &crs, None)
            .map_err(|e| e.to_refusal())?;
        let source_report = render::sources(&bundle, &tables, &anchors, &declaration);
        let projection = || -> Result<(String, String), Refusal> {
            let count = bundle
                .surveys
                .len()
                .saturating_add(bundle.assays.len().saturating_mul(2))
                .saturating_add(bundle.collars.len().saturating_mul(2));
            if count > MAX_OUTPUT_POSITIONS {
                return Err(invalid(format!(
                    "{count} positions exceed {MAX_OUTPUT_POSITIONS}; split the source tables on complete holes"
                )));
            }
            let holes = prepare_geometry(&bundle, &declaration.holes, &crs)?;
            if compositing {
                composite::render(&holes, &bundle, &anchors, &declaration)
            } else {
                render::geometry(holes, &anchors, &declaration)
            }
        };
        let mut envelope = match projection() {
            Ok((geometry, markdown)) => {
                let report = format!("{{{source_report},\"{key}\":{geometry}}}");
                let digest = fmine_core::hash::sha256(report.as_bytes());
                let mut envelope = Envelope::ok(
                    verb,
                    format!("{{\"receipt_sha256\":\"{digest}\",\"report\":{report}}}"),
                    Vec::new(),
                );
                envelope.rendered_markdown = Some(format!(
                    "# {title}\n\nEvidence lane: {}. Survey error model: {}. {evidence_description}\n\n{markdown}\n\nReceipt SHA-256: `{digest}`. Source bundle: `{}`.\n\nOmitted uncertainty: {}\n\n{no_claim}",
                    declaration.lane,
                    declaration.survey_error_model,
                    bundle.bundle_digest,
                    worksheet_input::markdown_cell(&declaration.omitted_uncertainty),
                ));
                envelope
            }
            Err(refusal) => {
                let mut envelope = Envelope::refused(verb, refusal, actions());
                envelope.payload_json = format!("{{{source_report},\"{key}\":null}}");
                envelope
            }
        };
        envelope.no_claim = Some(no_claim);
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| Envelope::refused(verb, refusal, actions()));
    envelope.no_claim = Some(no_claim);
    envelope
}
