//! DESIGN (BD-188, bd-zx57): explicit geometry declarations join quarantined acQuire exports to Corebox.
//! Determinism: DeterministicPerIsa for the rational-to-binary64 geometry projection.
//!
//! Original rational values, censoring, source row numbers and import receipts stay in the bundle. This
//! adapter does not invent instrument accuracy, convert bearings, create assay/custody records or write
//! authority. A bundle with any rejected row cannot become a complete-looking hole trace.

use std::collections::{BTreeMap, BTreeSet};

use fmine_drill::desurvey::CollarFrom;
use fmine_drill::records::{
    Collar, DrillRecord, HoleType, Orientation, Position, PositionMethod, RunDirection, Survey,
    SurveyInstrument,
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::ExactQty;

use super::{AcquireAssayInterval, AcquireExportBundle};
use crate::dialects::{ColumnBasis, ColumnSupport, ResolvedUnit};
use crate::quarantine::ImportReport;
use crate::redact::REDACTED_MARKER;

/// What north the supplied azimuths use. Only grid bearings need no unprovided correction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BearingReference {
    Grid,
    True,
    Magnetic,
}

/// Explicit per-hole metadata absent from the three supported acQuire tables.
/// Accuracy fields are one-sigma declarations, not values inferred from instrument names.
#[derive(Clone, Debug, PartialEq)]
pub struct HoleGeometry {
    pub hole_id: String,
    /// The declared drilling program; source PROJECTCODE is retained separately, not treated as this field.
    pub program: String,
    pub hole_type: HoleType,
    pub collar_from: CollarFrom,
    pub position_method: PositionMethod,
    pub sigma_horizontal_m: f64,
    pub sigma_vertical_m: f64,
    pub instrument: SurveyInstrument,
    pub run: RunDirection,
    pub sigma_azimuth_deg: f64,
    pub sigma_dip_deg: f64,
    pub magnetic_interference: bool,
    pub azimuth_reference: BearingReference,
    /// The exact source SURVEYTYPE label this declaration maps to `instrument`, when present.
    /// Otherwise a printed source label must equal the instrument code, ignoring ASCII case.
    /// Absent source labels remain absent; the instrument is an explicit supplied declaration.
    pub survey_type_label: Option<String>,
}

/// A source survey row projected into the Corebox geometry type.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSurvey {
    pub row_index: usize,
    pub record: Survey,
}

/// An assay interval's exact millimetre endpoints beside its unchanged source reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedAssay {
    pub from_mm: u64,
    pub to_mm: u64,
    pub source: AcquireAssayInterval,
}

/// A complete admitted hole's geometry and assay spans. This is not an authority-ledger commit.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedHole {
    pub collar: Collar,
    pub collar_row_index: usize,
    pub max_depth_mm: u64,
    pub surveys: Vec<PreparedSurvey>,
    pub assays: Vec<PreparedAssay>,
}

fn source_error(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::SourceCellUnusable208, detail)
}

fn physical_error(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn identity(value: &str, field: &str) -> Result<(), Refusal> {
    if value.is_empty() || value.trim() != value || value.contains(REDACTED_MARKER) {
        Err(source_error(format!(
            "acQuire {field}: empty, padded or redacted identity cannot identify a hole or sample"
        )))
    } else {
        Ok(())
    }
}

fn geometry_columns(report: &ImportReport, fields: &[(&str, &str)]) -> Result<(), Refusal> {
    for &(name, unit) in fields {
        let mut columns = report
            .resolved_columns
            .iter()
            .filter(|c| c.column_name == name);
        let Some(column) = columns.next() else {
            return Err(source_error(format!(
                "acQuire geometry missing column {name}"
            )));
        };
        if columns.next().is_some()
            || column.undeclared_in_dialect
            || !matches!(&column.unit, ResolvedUnit::Registered(u) if u == unit)
            || column.basis != Some(ColumnBasis::None)
            || column.support != Some(ColumnSupport::PointCore)
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "acQuire geometry {name} requires one declared {unit} column with basis none and point_core support"
                ),
            ));
        }
    }
    Ok(())
}

fn millimetres(value: ExactQty, field: &str) -> Result<u64, Refusal> {
    let mm = value
        .checked_mul(ExactQty::from_integer(1000))
        .map_err(|e| physical_error(format!("{field}: {e}")))?;
    // The downstream trace uses binary64. No integer depth past its exact-integer range is admitted.
    if mm.den() != 1 || mm.num() < 0 || mm.num() > (1_i128 << 53) {
        return Err(physical_error(format!(
            "{field} must be an exact nonnegative integer millimetre depth no greater than 2^53 mm"
        )));
    }
    u64::try_from(mm.num()).map_err(|e| physical_error(format!("{field}: {e}")))
}

fn coordinate(value: ExactQty, field: &str) -> Result<f64, Refusal> {
    let projected = value.num() as f64 / value.den() as f64;
    if projected.is_finite() && (value.signum() == 0 || projected != 0.0) {
        Ok(projected)
    } else {
        Err(physical_error(format!(
            "{field} has no finite non-underflowing binary64 geometry projection"
        )))
    }
}

fn metric_crs(crs: &str, registered: &BTreeSet<String>) -> Result<(), Refusal> {
    // Registered WGS84 UTM codes carry metres; a geographic or unknown local grid cannot inherit this.
    let code = crs
        .strip_prefix("EPSG:")
        .and_then(|s| s.parse::<u32>().ok());
    if registered.contains(crs) && matches!(code, Some(32601..=32660 | 32701..=32760)) {
        Ok(())
    } else {
        Err(Refusal::new(
            RefusalCode::CrsUnregisteredOrIncompatible106,
            format!("acQuire desurvey requires a registered WGS84 UTM metre CRS; got {crs}"),
        ))
    }
}

/// Join every admitted hole to explicit geometry metadata, preserving assay support and readings.
///
/// The bundle must come from the quarantine importer. Any source rejection refuses this entire projection;
/// the caller retains and renders the original import report. This avoids treating a hole with an omitted,
/// malformed survey station as complete. Declarations exactly cover the imported collars, and every collar
/// needs at least one survey. True/magnetic azimuths, unregistered or non-metric frames, implicit unit
/// conversions and sub-millimetre depth rounding refuse. No input is changed.
pub fn prepare_geometry(
    bundle: &AcquireExportBundle,
    declarations: &[HoleGeometry],
    registered_crs: &BTreeSet<String>,
) -> Result<Vec<PreparedHole>, Refusal> {
    if !bundle.rejected_rows.is_empty()
        || [
            &bundle.collar_report,
            &bundle.survey_report,
            &bundle.assay_report,
        ]
        .iter()
        .any(|report| !report.rejected_rows.is_empty())
    {
        return Err(source_error(
            "acQuire geometry refused: source rows were quarantined; inspect the retained import report before tracing holes",
        ));
    }
    if bundle.collars.is_empty() {
        return Err(source_error(
            "acQuire geometry requires at least one admitted collar",
        ));
    }
    metric_crs(&bundle.crs, registered_crs)?;
    geometry_columns(
        &bundle.collar_report,
        &[
            ("EAST", "m"),
            ("NORTH", "m"),
            ("RL", "m"),
            ("MAXDEPTH", "m"),
        ],
    )?;
    geometry_columns(
        &bundle.survey_report,
        &[("DEPTH", "m"), ("AZIMUTH", "deg"), ("DIP", "deg")],
    )?;
    geometry_columns(&bundle.assay_report, &[("FROM", "m"), ("TO", "m")])?;

    let mut metadata = BTreeMap::new();
    for declaration in declarations {
        identity(&declaration.hole_id, "declared hole_id")?;
        if metadata
            .insert(declaration.hole_id.as_str(), declaration)
            .is_some()
        {
            return Err(source_error("duplicate hole geometry declaration"));
        }
        if declaration.azimuth_reference != BearingReference::Grid {
            return Err(Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                "acQuire desurvey requires explicitly declared grid azimuths; no magnetic or true-north correction was supplied",
            ));
        }
        if declaration.instrument == SurveyInstrument::None {
            return Err(source_error(
                "an unsurveyed hole cannot become an assumed straight hole",
            ));
        }
        if declaration.program.trim().is_empty()
            || declaration
                .survey_type_label
                .as_ref()
                .is_some_and(|s| s.trim().is_empty() || s.trim() != s)
        {
            return Err(source_error(
                "geometry program and supplied survey-type labels must be nonempty",
            ));
        }
    }

    let mut holes = BTreeMap::new();
    for source in &bundle.collars {
        identity(&source.hole_id, "collar HOLEID")?;
        if source.crs != bundle.crs {
            return Err(Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                "a collar's CRS differs from its imported bundle",
            ));
        }
        let declared = metadata.remove(source.hole_id.as_str()).ok_or_else(|| {
            source_error(format!(
                "missing or duplicate geometry declaration for collar row {}",
                source.row_index
            ))
        })?;
        let position = Position {
            crs: source.crs.clone(),
            easting_m: coordinate(source.easting_m, "EAST")?,
            northing_m: coordinate(source.northing_m, "NORTH")?,
            elevation_m: coordinate(source.rl_m, "RL")?,
            method: declared.position_method,
            sigma_horizontal_m: declared.sigma_horizontal_m,
            sigma_vertical_m: declared.sigma_vertical_m,
        };
        let (planned, actual) = match declared.collar_from {
            CollarFrom::Actual => (None, Some(position)),
            CollarFrom::Planned => (Some(position), None),
        };
        let collar = Collar {
            hole_id: source.hole_id.clone(),
            hole_type: declared.hole_type,
            program: declared.program.clone(),
            planned,
            actual,
        };
        DrillRecord::Collar(collar.clone())
            .validate()
            .map_err(|e| e.to_refusal())?;
        let max_depth_mm = millimetres(source.max_depth_m, "MAXDEPTH")?;
        if max_depth_mm == 0 {
            return Err(physical_error("MAXDEPTH must be positive"));
        }
        holes.insert(
            source.hole_id.clone(),
            (
                declared,
                PreparedHole {
                    collar,
                    collar_row_index: source.row_index,
                    max_depth_mm,
                    surveys: Vec::new(),
                    assays: Vec::new(),
                },
            ),
        );
    }
    if !metadata.is_empty() {
        return Err(source_error(
            "geometry declaration names a hole absent from the imported collars",
        ));
    }

    let mut survey_rows = BTreeSet::new();
    for source in &bundle.surveys {
        identity(&source.hole_id, "survey HOLEID")?;
        if !survey_rows.insert(source.row_index) {
            return Err(source_error("duplicate survey source row"));
        }
        let (declared, hole) = holes
            .get_mut(&source.hole_id)
            .ok_or_else(|| source_error("survey has no admitted collar"))?;
        if let Some(label) = &source.survey_type {
            let agrees = match &declared.survey_type_label {
                Some(expected) => label == expected,
                None => label.eq_ignore_ascii_case(declared.instrument.code()),
            };
            if !agrees {
                return Err(source_error(format!(
                    "survey row {} SURVEYTYPE does not match its explicit instrument declaration",
                    source.row_index
                )));
            }
        }
        let depth_mm = millimetres(source.depth_m, "DEPTH")?;
        if depth_mm > hole.max_depth_mm {
            return Err(physical_error("survey DEPTH exceeds MAXDEPTH"));
        }
        let record = Survey {
            survey_id: format!(
                "acquire:{}:{}",
                bundle.survey_report.source_sha256, source.row_index
            ),
            hole_id: source.hole_id.clone(),
            depth_mm,
            instrument: declared.instrument,
            orientation: Some(Orientation {
                azimuth_deg: coordinate(source.azimuth_deg, "AZIMUTH")?,
                dip_deg: coordinate(source.dip_deg, "DIP")?,
                sigma_azimuth_deg: declared.sigma_azimuth_deg,
                sigma_dip_deg: declared.sigma_dip_deg,
            }),
            magnetic_interference: declared.magnetic_interference,
            run: declared.run,
        };
        DrillRecord::Survey(record.clone())
            .validate()
            .map_err(|e| e.to_refusal())?;
        hole.surveys.push(PreparedSurvey {
            row_index: source.row_index,
            record,
        });
    }
    let mut assay_rows = BTreeSet::new();
    for source in &bundle.assays {
        identity(&source.hole_id, "assay HOLEID")?;
        if let Some(id) = &source.sample_id {
            identity(id, "SAMPLEID")?;
        }
        if !assay_rows.insert(source.row_index) {
            return Err(source_error("duplicate assay source row"));
        }
        let (_, hole) = holes
            .get_mut(&source.hole_id)
            .ok_or_else(|| source_error("assay has no admitted collar"))?;
        let from_mm = millimetres(source.from_m, "FROM")?;
        let to_mm = millimetres(source.to_m, "TO")?;
        if from_mm >= to_mm || to_mm > hole.max_depth_mm {
            return Err(physical_error("assay requires 0 <= FROM < TO <= MAXDEPTH"));
        }
        hole.assays.push(PreparedAssay {
            from_mm,
            to_mm,
            source: source.clone(),
        });
    }
    holes
        .into_values()
        .map(|(_, mut hole)| {
            hole.surveys.sort_by_key(|s| s.record.depth_mm);
            if hole.surveys.is_empty()
                || hole
                    .surveys
                    .windows(2)
                    .any(|w| w[0].record.depth_mm == w[1].record.depth_mm)
            {
                return Err(source_error(
                    "every hole requires nonempty, unique survey depths",
                ));
            }
            hole.assays
                .sort_by_key(|a| (a.from_mm, a.to_mm, a.source.row_index));
            if hole.assays.windows(2).any(|w| w[0].to_mm > w[1].from_mm) {
                return Err(Refusal::new(
                    RefusalCode::SupportMismatch102,
                    "assay intervals overlap in the geometry projection",
                ));
            }
            Ok(hole)
        })
        .collect()
}
