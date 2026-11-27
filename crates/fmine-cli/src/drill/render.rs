//! Source receipts and typed geometry Claims share the same captured inputs.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_drill::desurvey;
use fmine_drill::survey_error::{self, Angle, Covariance, ErrorModel, Propagation, UncertainTrace};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal};
use fmine_io::acquire::{AcquireExportBundle, AssayReading, drill::PreparedHole};
use fmine_io::quarantine::ImportReport;
use fmine_units::ExactQty;

use super::{
    SourceTable,
    manifest::{Manifest, SCHEMA, invalid},
};
use crate::{escape_json, worksheet_input::markdown_cell};

fn quoted(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), quoted)
}

fn exact(value: ExactQty) -> String {
    quoted(&if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    })
}

fn anchors_json(anchors: &[SourceAnchor]) -> String {
    anchors
        .iter()
        .map(|source| {
            format!(
                "{{\"path\":{},\"sha256\":\"{}\",\"locator\":{}}}",
                quoted(&source.artifact_path),
                Digest32(source.artifact_sha256),
                quoted(&source.locator),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn table_json(role: &str, source: &SourceTable, report: &ImportReport) -> String {
    format!(
        "{{\"role\":{},\"path\":{},\"format\":{},\"sheet\":{},\"sha256\":{},\"cas_root\":\"{}\",\"bytes\":{},\"dialect\":{},\"quarantine_digest\":\"{}\",\"data_rows\":{},\"quarantine_admitted_rows\":{},\"quarantine_rejected_rows\":{},\"personal_columns_redacted\":{},\"inline_tokens_redacted\":{}}}",
        quoted(role),
        quoted(&source.source.artifact_path),
        quoted(&source.format),
        optional(source.sheet.as_deref()),
        quoted(&report.source_sha256),
        report.source_cas_root,
        report.source_bytes,
        quoted(&report.dialect_id),
        report.report_digest,
        report.total_data_rows,
        report.admitted_rows.len(),
        report.rejected_rows.len(),
        report.total_columns_redacted,
        report.total_inline_pii_scrubbed,
    )
}

pub(super) fn sources(
    bundle: &AcquireExportBundle,
    tables: &[SourceTable],
    anchors: &[SourceAnchor],
    declaration: &Manifest,
) -> String {
    let tables = tables
        .iter()
        .map(|table| {
            let report = match table.role.as_str() {
                "collar" => &bundle.collar_report,
                "survey" => &bundle.survey_report,
                _ => &bundle.assay_report,
            };
            table_json(&table.role, table, report)
        })
        .collect::<Vec<_>>()
        .join(",");
    let rejections = bundle.rejected_rows.iter().map(|row| format!(
        "{{\"table\":{},\"row\":{},\"column\":{},\"value\":{},\"code\":{},\"what\":{},\"fix\":{}}}",
        quoted(row.table.as_str()), row.row_index, quoted(&row.column), quoted(&row.raw_value),
        quoted(row.code), quoted(&row.explanation), quoted(row.fix),
    )).collect::<Vec<_>>().join(",");
    format!(
        "\"schema\":{},\"evidence_lane\":{},\"bundle_digest\":\"{}\",\"crs\":{},\"omitted_uncertainty\":{},\"sources\":[{}],\"tables\":[{tables}],\"domain_admitted\":{{\"collars\":{},\"surveys\":{},\"assays\":{}}},\"rejected_rows\":[{rejections}]",
        quoted(SCHEMA),
        quoted(&declaration.lane),
        bundle.bundle_digest,
        quoted(&bundle.crs),
        quoted(&declaration.omitted_uncertainty),
        anchors_json(anchors),
        bundle.collars.len(),
        bundle.surveys.len(),
        bundle.assays.len(),
    )
}

fn covariance_json(covariance: &Covariance) -> Result<String, Refusal> {
    if covariance.enu_m2.iter().flatten().any(|v| !v.is_finite()) {
        return Err(invalid("non-finite desurvey covariance"));
    }
    Ok(format!(
        "[{}]",
        covariance
            .enu_m2
            .iter()
            .map(|row| format!("[{},{},{}]", row[0], row[1], row[2]))
            .collect::<Vec<_>>()
            .join(","),
    ))
}

fn position(trace: &UncertainTrace, depth_mm: u64) -> Result<String, Refusal> {
    let located = trace.trace().at(depth_mm);
    let position_covariance = trace
        .checked_covariance_at(depth_mm)
        .map_err(|e| e.to_refusal())?;
    let point = [
        located.point.easting_m,
        located.point.northing_m,
        located.point.elevation_m,
    ];
    let direction = [
        located.direction.east,
        located.direction.north,
        located.direction.up,
    ];
    if point.iter().chain(direction.iter()).any(|v| !v.is_finite()) {
        return Err(invalid("non-finite desurvey coordinate or direction"));
    }
    let matrix = covariance_json(&position_covariance.total)?;
    let collar = covariance_json(&position_covariance.collar)?;
    let terms = position_covariance
        .terms
        .iter()
        .map(|term| {
            Ok(format!(
                "{{\"code\":{},\"covariance_m2\":{}}}",
                quoted(&term.code),
                covariance_json(&term.covariance)?,
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    Ok(format!(
        "{{\"depth_mm\":{depth_mm},\"point_m\":[{},{},{}],\"direction_enu\":[{},{},{}],\"covariance_m2\":{matrix},\"collar_covariance_m2\":{collar},\"covariance_terms_m2\":[{terms}],\"extrapolated\":{}}}",
        point[0],
        point[1],
        point[2],
        direction[0],
        direction[1],
        direction[2],
        located.extrapolated,
    ))
}

fn reading(reading: &AssayReading) -> String {
    match reading {
        AssayReading::Measured { value, unit, basis } => format!(
            "{{\"state\":\"measured_token\",\"value\":{},\"unit\":{},\"basis\":{}}}",
            exact(*value),
            quoted(unit),
            quoted(&format!("{basis:?}")),
        ),
        AssayReading::BelowDetection {
            detection_limit,
            unit,
            basis,
            raw,
        } => format!(
            "{{\"state\":\"below_detection\",\"limit\":{},\"unit\":{},\"basis\":{},\"raw\":{}}}",
            exact(*detection_limit),
            quoted(unit),
            quoted(&format!("{basis:?}")),
            quoted(raw),
        ),
        AssayReading::Placeholder { raw } => {
            format!("{{\"state\":\"placeholder\",\"raw\":{}}}", quoted(raw),)
        }
    }
}

struct HoleResult {
    source: PreparedHole,
    trace: UncertainTrace,
}

fn hole_json(hole: &HoleResult) -> Result<String, Refusal> {
    let rows: BTreeMap<_, _> = hole
        .source
        .surveys
        .iter()
        .map(|s| (s.record.survey_id.as_str(), s.row_index))
        .collect();
    let stations = hole
        .trace
        .trace()
        .stations()
        .iter()
        .map(|station| {
            let row = rows
                .get(station.survey_id.as_str())
                .ok_or_else(|| invalid("station lost its source row"))?;
            Ok(format!(
                "{{\"source_row\":{row},\"survey_id\":{},\"suspect\":{},\"position\":{}}}",
                quoted(&station.survey_id),
                station.suspect,
                position(&hole.trace, station.depth_mm)?,
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let assays = hole.source.assays.iter().map(|assay| {
        let source = &assay.source;
        let readings = source.assays.iter().map(|(key, value)| {
            format!("{}:{}", quoted(key), reading(value))
        }).collect::<Vec<_>>().join(",");
        Ok(format!(
            "{{\"source_row\":{},\"sample_id\":{},\"from_m_exact\":{},\"to_m_exact\":{},\"lab_job\":{},\"method\":{},\"readings\":{{{readings}}},\"start\":{},\"end\":{}}}",
            source.row_index, optional(source.sample_id.as_deref()), exact(source.from_m), exact(source.to_m),
            optional(source.lab_job.as_deref()), optional(source.method.as_deref()),
            position(&hole.trace, assay.from_mm)?, position(&hole.trace, assay.to_mm)?,
        ))
    }).collect::<Result<Vec<_>, Refusal>>()?.join(",");
    Ok(format!(
        "{{\"hole_id\":{},\"program\":{},\"hole_type\":{},\"collar_source_row\":{},\"collar_from\":{},\"max_depth_mm\":{},\"collar\":{},\"bottom\":{},\"stations\":[{stations}],\"assays\":[{assays}]}}",
        quoted(&hole.source.collar.hole_id),
        quoted(&hole.source.collar.program),
        quoted(hole.source.collar.hole_type.code()),
        hole.source.collar_row_index,
        quoted(match hole.trace.trace().collar_from() {
            desurvey::CollarFrom::Actual => "actual",
            desurvey::CollarFrom::Planned => "planned",
        }),
        hole.source.max_depth_mm,
        position(&hole.trace, 0)?,
        position(&hole.trace, hole.source.max_depth_mm)?,
    ))
}

pub(super) fn geometry(
    holes: Vec<PreparedHole>,
    anchors: &[SourceAnchor],
    declaration: &Manifest,
) -> Result<(String, String), Refusal> {
    let model = match declaration.survey_error_model.as_str() {
        "random" => ErrorModel::random(),
        "systematic" => ErrorModel::systematic(),
        _ => return Err(invalid("unknown survey error model")),
    };
    let terms = model
        .terms
        .iter()
        .map(|term| {
            format!(
                "{{\"code\":{},\"angle\":{},\"propagation\":{},\"variance_share\":{}}}",
                quoted(&term.code),
                quoted(match term.angle {
                    Angle::Azimuth => "azimuth",
                    Angle::Dip => "dip",
                }),
                quoted(match term.propagation {
                    Propagation::Random => "random",
                    Propagation::Systematic => "systematic",
                }),
                term.variance_share,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut rendered = Vec::new();
    let mut md = String::from(
        "| Hole | Stations | Assays | Bottom depth (m) | Bottom east / north / up (m) | One-sigma east / north / up (m) | Extrapolated |\n|---|---:|---:|---:|---|---|---|\n",
    );
    for source in holes {
        let surveys = source
            .surveys
            .iter()
            .map(|s| s.record.clone())
            .collect::<Vec<_>>();
        let trace = survey_error::propagate(&source.collar, &surveys, &model)
            .map_err(|e| e.to_refusal())?;
        let result = HoleResult { source, trace };
        let body = hole_json(&result)?;
        let claim = Claim::new(
            "drill.desurvey/1",
            result,
            &body,
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::internal(),
            vec![],
            vec![],
            anchors.to_vec(),
        );
        claim.check_id()?;
        let hole = &claim.value;
        let depth = hole.source.max_depth_mm;
        let bottom = hole.trace.trace().at(depth);
        let sigma = hole
            .trace
            .checked_covariance_at(depth)
            .map_err(|e| e.to_refusal())?
            .total
            .sigma_m();
        let _ = writeln!(
            md,
            "| {} | {} | {} | {:.3} | {:.3} / {:.3} / {:.3} | {:.3} / {:.3} / {:.3} | {} |",
            markdown_cell(&hole.source.collar.hole_id),
            hole.source.surveys.len(),
            hole.source.assays.len(),
            depth as f64 / 1000.0,
            bottom.point.easting_m,
            bottom.point.northing_m,
            bottom.point.elevation_m,
            sigma[0],
            sigma[1],
            sigma[2],
            bottom.extrapolated,
        );
        rendered.push(format!(
            "{{\"claim\":{{\"id\":{},\"metric\":{},\"evidence\":\"Assumed\",\"state\":\"Estimated\",\"calibration\":\"uncalibrated\",\"confidentiality\":\"Internal\",\"support\":\"point_core_geometry\",\"additivity\":\"NonAdditive\",\"sources\":[{}]}},\"result\":{body}}}",
            quoted(&claim.claim_id.to_string()), quoted(&claim.metric_key), anchors_json(&claim.anchors),
        ));
    }
    let payload = format!(
        "{{\"error_model\":{},\"error_terms\":[{terms}],\"coordinate_order\":[\"east\",\"north\",\"up\"],\"covariance_unit\":\"m2\",\"horizontal_sigma_basis\":\"per_grid_axis\",\"evidence_lane\":{},\"holes\":[{}]}}",
        quoted(&declaration.survey_error_model),
        quoted(&declaration.lane),
        rendered.join(","),
    );
    md.push_str("\nJSON retains every station and assay endpoint with its full covariance, source row and Claim. Censored and missing assays remain censored and missing. Extrapolation and magnetic-interference flags remain visible.\n");
    Ok((payload, md))
}
