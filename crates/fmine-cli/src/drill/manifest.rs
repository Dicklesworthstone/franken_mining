//! Explicit source and survey-error declarations for the Corebox command.
//! Determinism: Exact

use std::collections::BTreeMap;
use std::path::PathBuf;

use fmine_drill::{
    desurvey::CollarFrom,
    records::{HoleType, PositionMethod, RunDirection, SurveyInstrument},
};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::acquire::drill::{BearingReference, HoleGeometry};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

pub(super) const SCHEMA: &str = "fmine.drill-desurvey.v1";

pub(super) struct Table {
    pub role: String,
    pub path: PathBuf,
    pub format: String,
    pub sheet: Option<String>,
}

pub(super) struct Manifest {
    pub schema: &'static str,
    pub crs: String,
    pub lane: String,
    pub survey_error_model: String,
    pub omitted_uncertainty: String,
    pub tables: Vec<Table>,
    pub holes: Vec<HoleGeometry>,
    pub compositing: Option<super::composite::Declaration>,
}

pub(super) fn invalid(detail: impl Into<String>) -> Refusal {
    Refusal::new(
        RefusalCode::SourceCellUnusable208,
        format!("drill declaration: {}", detail.into()),
    )
}

pub(super) fn fields<'a>(
    values: &'a [TomlField],
    required: &[&str],
    optional: &[&str],
) -> Result<BTreeMap<&'a str, &'a str>, Refusal> {
    let mut result = BTreeMap::new();
    for field in values {
        if !required.contains(&field.key.as_str()) && !optional.contains(&field.key.as_str()) {
            return Err(invalid(format!(
                "unknown field {} at line {}",
                field.key, field.line
            )));
        }
        let TomlValue::Str(value) = &field.value else {
            return Err(invalid(format!("{} must be a quoted string", field.key)));
        };
        if value.trim().is_empty() {
            return Err(invalid(format!("{} must not be empty", field.key)));
        }
        result.insert(field.key.as_str(), value.as_str());
    }
    for key in required {
        if !result.contains_key(key) {
            return Err(invalid(format!("missing {key}")));
        }
    }
    Ok(result)
}

pub(super) fn get<'a>(values: &BTreeMap<&str, &'a str>, key: &str) -> Result<&'a str, Refusal> {
    values
        .get(key)
        .copied()
        .ok_or_else(|| invalid(format!("missing {key}")))
}

fn sigma(values: &BTreeMap<&str, &str>, key: &str) -> Result<f64, Refusal> {
    let raw = get(values, key)?;
    let nonzero_mantissa = raw
        .split(['e', 'E'])
        .next()
        .is_some_and(|s| s.bytes().any(|b| (b'1'..=b'9').contains(&b)));
    raw.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0 && (*v != 0.0 || !nonzero_mantissa))
        .ok_or_else(|| {
            invalid(format!(
                "{key} must be a finite nonnegative one-sigma value without nonzero underflow"
            ))
        })
}

fn hole(values: &[TomlField]) -> Result<HoleGeometry, Refusal> {
    let f = fields(
        values,
        &[
            "hole_id",
            "program",
            "hole_type",
            "collar_from",
            "position_method",
            "sigma_horizontal_m",
            "sigma_vertical_m",
            "instrument",
            "run",
            "sigma_azimuth_deg",
            "sigma_dip_deg",
            "magnetic_interference",
            "azimuth_reference",
        ],
        &["survey_type_label"],
    )?;
    let hole_type = get(&f, "hole_type")?;
    let position_method = get(&f, "position_method")?;
    let instrument = get(&f, "instrument")?;
    let run = get(&f, "run")?;
    Ok(HoleGeometry {
        hole_id: get(&f, "hole_id")?.into(),
        program: get(&f, "program")?.into(),
        hole_type: HoleType::ALL
            .iter()
            .copied()
            .find(|v| v.code() == hole_type)
            .ok_or_else(|| invalid("unknown hole_type"))?,
        collar_from: match get(&f, "collar_from")? {
            "actual" => CollarFrom::Actual,
            "planned" => CollarFrom::Planned,
            _ => return Err(invalid("collar_from must be actual or planned")),
        },
        position_method: PositionMethod::ALL
            .iter()
            .copied()
            .find(|v| v.code() == position_method)
            .ok_or_else(|| invalid("unknown position_method"))?,
        sigma_horizontal_m: sigma(&f, "sigma_horizontal_m")?,
        sigma_vertical_m: sigma(&f, "sigma_vertical_m")?,
        instrument: SurveyInstrument::ALL
            .iter()
            .copied()
            .find(|v| v.code() == instrument)
            .ok_or_else(|| invalid("unknown instrument"))?,
        run: RunDirection::ALL
            .iter()
            .copied()
            .find(|v| v.code() == run)
            .ok_or_else(|| invalid("run must be in or out"))?,
        sigma_azimuth_deg: sigma(&f, "sigma_azimuth_deg")?,
        sigma_dip_deg: sigma(&f, "sigma_dip_deg")?,
        magnetic_interference: match get(&f, "magnetic_interference")? {
            "true" => true,
            "false" => false,
            _ => {
                return Err(invalid(
                    "magnetic_interference must be quoted true or false",
                ));
            }
        },
        azimuth_reference: match get(&f, "azimuth_reference")? {
            "grid" => BearingReference::Grid,
            "true" => BearingReference::True,
            "magnetic" => BearingReference::Magnetic,
            _ => return Err(invalid("azimuth_reference must be grid, true or magnetic")),
        },
        survey_type_label: f.get("survey_type_label").map(|s| (*s).to_owned()),
    })
}

pub(super) fn parse(text: &str, composite: bool) -> Result<Manifest, Refusal> {
    let doc = parse_subset(text).map_err(|e| invalid(e.to_string()))?;
    let schema = if composite {
        super::composite::SCHEMA
    } else {
        SCHEMA
    };
    let top = fields(
        &doc.top,
        &[
            "schema",
            "crs",
            "evidence_lane",
            "survey_error_model",
            "omitted_uncertainty",
        ],
        &[],
    )?;
    if get(&top, "schema")? != schema {
        return Err(invalid(format!("schema must be {schema}")));
    }
    let lane = get(&top, "evidence_lane")?;
    if !matches!(lane, "synthetic" | "internal") {
        return Err(invalid("evidence_lane must be synthetic or internal"));
    }
    let survey_error_model = get(&top, "survey_error_model")?;
    if !matches!(survey_error_model, "random" | "systematic") {
        return Err(invalid("survey_error_model must be random or systematic"));
    }
    if doc.rows.len() > if composite { 24_100 } else { 4099 } {
        return Err(invalid(
            "at most three tables and 4096 hole declarations are admitted",
        ));
    }
    let mut tables = BTreeMap::new();
    let mut holes = Vec::new();
    for row in &doc.rows {
        match row.table.as_str() {
            "table" => {
                let f = fields(&row.fields, &["role", "path", "format"], &["sheet"])?;
                let role = get(&f, "role")?;
                let format = get(&f, "format")?;
                if !matches!(role, "collar" | "survey" | "assay") {
                    return Err(invalid("table role must be collar, survey or assay"));
                }
                if !matches!(format, "csv" | "xlsx") {
                    return Err(invalid("table format must be csv or xlsx"));
                }
                let sheet = f.get("sheet").map(|s| (*s).to_owned());
                if (format == "xlsx") != sheet.is_some() {
                    return Err(invalid(
                        "an xlsx table requires an explicit sheet; a csv table has none",
                    ));
                }
                let table = Table {
                    role: role.into(),
                    path: get(&f, "path")?.into(),
                    format: format.into(),
                    sheet,
                };
                if tables.insert(role.to_owned(), table).is_some() {
                    return Err(invalid(format!("duplicate {role} table")));
                }
            }
            "hole" => holes.push(hole(&row.fields)?),
            "composite" | "domain" if composite => {}
            _ => {
                return Err(invalid(format!(
                    "unsupported table {} at line {}",
                    row.table, row.line
                )));
            }
        }
    }
    if tables.len() != 3 || holes.is_empty() || holes.len() > 4096 {
        return Err(invalid(
            "exactly one collar, survey and assay table, and 1..4096 holes are required",
        ));
    }
    Ok(Manifest {
        schema,
        crs: get(&top, "crs")?.into(),
        lane: lane.into(),
        survey_error_model: survey_error_model.into(),
        omitted_uncertainty: get(&top, "omitted_uncertainty")?.into(),
        tables: tables.into_values().collect(),
        holes,
        compositing: if composite {
            Some(super::composite::parse(&doc)?)
        } else {
            None
        },
    })
}
