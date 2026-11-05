//! DESIGN: explicit whole-material worksheets into exact nominal elemental kg (BD-177/336).
//! Submitted source values and support declarations remain Assumed and Internal.
//! Determinism: Exact

use std::path::Path;

use fmine_claim::Claim;
use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_plant::{
    flowsheet::ElementKey,
    measurements::{
        self, AssayUnit, MassUnit, MaterialMeasurement, MeasurementScope, MeasurementValue,
    },
};
use fmine_registry::toml_subset::parse_subset;
use fmine_time::ValidTime;

use super::{bad, claim_json, declared, exact, fields, get, integer, quantity, quoted};
use crate::{Envelope, NextAction, worksheet_input};

const SCHEMA: &str = "fmine.plant-material.v1";
const ASSAY_BASIS: &str = "total_element/all_non_water_dry_matter";
const MAX_BYTES: u64 = 1024 * 1024;

pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.plant-material.v1\",",
    "\"command\":\"plant project --input <worksheet.toml>\",",
    "\"input\":\"Regular UTF-8 TOML, at most 1 MiB and 256 rows; all values quoted; exact submitted bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"boundary\",\"material\",\"start_ns\",\"end_ns\",\"assay_basis\",\"whole_material_support\"],",
    "\"assay_basis\":\"total_element/all_non_water_dry_matter\",",
    "\"whole_material_support\":\"Must be true: every row covers the full declared material and the same physical time; no sample-to-material extrapolation\",",
    "\"times\":\"Quoted signed integer nanoseconds on one declared axis; an instant is allowed for a stocktake\",",
    "\"mass_rows\":[\"role\",\"value\",\"unit\"],",
    "\"mass_roles\":[\"dry\",\"wet\",\"water\"],",
    "\"mass_units\":[\"kg\",\"t\"],",
    "\"mass_routes\":\"Exactly dry + water, or wet + moisture; explicit zero water is required for a dry material\",",
    "\"moisture_rows\":[\"water_over_wet_mass\"],",
    "\"moisture\":\"H2O mass / total wet mass as a fraction in [0,1), never a percentage or water/dry ratio\",",
    "\"assay_rows\":[\"element\",\"measurand\",\"value\",\"unit\"],",
    "\"assay_units\":[\"mass_fraction\",\"mass_percent\",\"g_per_t\"],",
    "\"assays\":\"At least one unique total-element assay of all non-water dry matter, including dissolved dry residue; no separated-solid or partial/speciated assays; H/O require explicit water stoichiometry and refuse here\",",
    "\"quantities\":\"Exact plain decimals or n/d with a positive denominator; raw tokens and declared units are retained\",",
    "\"output\":\"Original source Claims; exact dry-matter kg, H2O kg and elemental kg; admitted Claim and receipt digest; failed projection retains inputs with a null result\",",
    "\"evidence\":\"Assumed, Internal and uncalibrated; no uncertainty propagation, recovery estimate, chemical conversion or P754 compliance\",",
    "\"example\":\"tests/plant/material_projection.toml\"}"
);

struct Observation {
    claim: Claim<MaterialMeasurement>,
    raw_value: String,
}

fn mass_unit(value: (&str, usize)) -> Result<MassUnit, Refusal> {
    match value.0 {
        "kg" => Ok(MassUnit::Kilograms),
        "t" => Ok(MassUnit::Tonnes),
        _ => Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "plant material line {}: mass unit must be kg or metric t, not {}",
                value.1, value.0
            ),
        )),
    }
}

fn assay_unit(value: (&str, usize)) -> Result<AssayUnit, Refusal> {
    match value.0 {
        "mass_fraction" => Ok(AssayUnit::MassFraction),
        "mass_percent" => Ok(AssayUnit::MassPercent),
        "g_per_t" => Ok(AssayUnit::GramsPerTonne),
        _ => Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "plant material line {}: total dry-basis assay unit must be mass_fraction, mass_percent or g_per_t, not {}",
                value.1, value.0
            ),
        )),
    }
}

fn parse(text: &str, source: &SourceAnchor) -> Result<Vec<Observation>, Refusal> {
    let document = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if document.rows.len() > 256 {
        return Err(bad(1, "a material worksheet admits at most 256 rows"));
    }
    let top = fields(
        &document.top,
        &[
            "schema",
            "boundary",
            "material",
            "start_ns",
            "end_ns",
            "assay_basis",
            "whole_material_support",
        ],
        &[],
        1,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "assay_basis")?.0 != ASSAY_BASIS {
        return Err(Refusal::new(
            RefusalCode::BasisMismatch105,
            format!(
                "plant material line {}: assay_basis must explicitly be {ASSAY_BASIS}; wet-basis, separated-solids and partial/speciated assays need a different model",
                get(&top, "assay_basis")?.1
            ),
        ));
    }
    if get(&top, "whole_material_support")?.0 != "true" {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "declare whole_material_support = \"true\" only when every mass, moisture and assay covers the declared full material and time; no change-of-support model is inferred",
        ));
    }
    let scope = MeasurementScope {
        boundary: get(&top, "boundary")?.0.into(),
        material: get(&top, "material")?.0.into(),
        physical_time: ValidTime {
            start_ns: integer(get(&top, "start_ns")?)?,
            end_ns: integer(get(&top, "end_ns")?)?,
        },
    };
    let mut observations = Vec::with_capacity(document.rows.len());
    for row in &document.rows {
        let (value, raw) = match row.table.as_str() {
            "mass" => {
                let values = fields(&row.fields, &["role", "value", "unit"], &[], row.line)?;
                let raw = get(&values, "value")?;
                let value = quantity(raw)?;
                let unit = mass_unit(get(&values, "unit")?)?;
                let role = get(&values, "role")?;
                let value = match role.0 {
                    "dry" => MeasurementValue::DryMass { value, unit },
                    "wet" => MeasurementValue::WetMass { value, unit },
                    "water" => MeasurementValue::WaterMass { value, unit },
                    _ => return Err(bad(role.1, "mass role must be dry, wet or water")),
                };
                (value, raw)
            }
            "moisture" => {
                let values = fields(&row.fields, &["water_over_wet_mass"], &[], row.line)?;
                let raw = get(&values, "water_over_wet_mass")?;
                (
                    MeasurementValue::Moisture {
                        water_over_wet_mass: quantity(raw)?,
                    },
                    raw,
                )
            }
            "assay" => {
                let values = fields(
                    &row.fields,
                    &["element", "measurand", "value", "unit"],
                    &[],
                    row.line,
                )?;
                let raw = get(&values, "value")?;
                (
                    MeasurementValue::DryAssay {
                        element: ElementKey {
                            element: get(&values, "element")?.0.into(),
                            measurand: get(&values, "measurand")?.0.into(),
                        },
                        value: quantity(raw)?,
                        unit: assay_unit(get(&values, "unit")?)?,
                    },
                    raw,
                )
            }
            _ => return Err(bad(row.line, format!("unsupported table {}", row.table))),
        };
        let additivity = value.additivity();
        let input = MaterialMeasurement {
            scope: scope.clone(),
            value,
        };
        let tag = input.canonical_tag();
        observations.push(Observation {
            claim: declared(
                input,
                &tag,
                measurements::INPUT_METRIC,
                additivity,
                source,
                format!(
                    "{} row line {}, value line {}; whole-material support line {}, total dry-basis assay declaration line {}",
                    row.table,
                    row.line,
                    raw.1,
                    get(&top, "whole_material_support")?.1,
                    get(&top, "assay_basis")?.1,
                ),
            ),
            raw_value: raw.0.into(),
        });
    }
    Ok(observations)
}

fn observation_json(input: &Observation) -> String {
    let value = &input.claim.value.value;
    let (role, value, unit, element, measurand) = match value {
        MeasurementValue::DryMass { value, unit }
        | MeasurementValue::WetMass { value, unit }
        | MeasurementValue::WaterMass { value, unit } => {
            let role = match &input.claim.value.value {
                MeasurementValue::DryMass { .. } => "dry_mass",
                MeasurementValue::WetMass { .. } => "wet_mass",
                _ => "water_mass",
            };
            let unit = match unit {
                MassUnit::Kilograms => "kg",
                MassUnit::Tonnes => "t",
            };
            (role, *value, unit, "null".into(), "null".into())
        }
        MeasurementValue::Moisture {
            water_over_wet_mass,
        } => (
            "moisture",
            *water_over_wet_mass,
            "water_over_wet_mass",
            "null".into(),
            "null".into(),
        ),
        MeasurementValue::DryAssay {
            element,
            value,
            unit,
        } => (
            "dry_assay",
            *value,
            match unit {
                AssayUnit::MassFraction => "mass_fraction",
                AssayUnit::MassPercent => "mass_percent",
                AssayUnit::GramsPerTonne => "g_per_t",
            },
            quoted(&element.element),
            quoted(&element.measurand),
        ),
    };
    format!(
        "{{\"role\":{},\"raw_value\":{},\"value\":{},\"unit\":{},\"element\":{element},\"measurand\":{measurand},\"claim\":{}}}",
        quoted(role),
        quoted(&input.raw_value),
        exact(value),
        quoted(unit),
        claim_json(&input.claim),
    )
}

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect the explicit plant material, moisture and total-element assay bases",
    )]
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) =
            worksheet_input::read(&root.join(input), "plant material worksheet", MAX_BYTES)?;
        let observations = parse(&text, &source)?;
        let claims: Vec<_> = observations.iter().map(|row| row.claim.clone()).collect();
        let scope = claims.first().map(|input| &input.value.scope);
        let scope_json = scope.map_or_else(
            || "null".into(),
            |scope| {
                format!(
                    "{{\"boundary\":{},\"material\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"whole_material_support\":true,\"assay_basis\":{}}}",
                    quoted(&scope.boundary),
                    quoted(&scope.material),
                    scope.physical_time.start_ns,
                    scope.physical_time.end_ns,
                    quoted(ASSAY_BASIS),
                )
            },
        );
        let (mut envelope, admitted, digest, result) = match measurements::project_material(&claims)
        {
            Ok(receipt) => {
                let amounts = &receipt.summary().amounts;
                let elements = amounts
                    .elements_kg
                    .iter()
                    .map(|(key, mass)| {
                        format!(
                            "{{\"element\":{},\"measurand\":{},\"kg\":{}}}",
                            quoted(&key.element),
                            quoted(&key.measurand),
                            exact(*mass),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                (
                    Envelope::ok("plant project", String::new(), vec![]),
                    claim_json(receipt.claim()),
                    quoted(receipt.digest()),
                    format!(
                        "{{\"dry_matter_kg\":{},\"water_kg\":{},\"elements_kg\":[{elements}]}}",
                        exact(amounts.dry_solids_kg),
                        exact(amounts.water_kg),
                    ),
                )
            }
            Err(refusal) => (
                Envelope::refused("plant project", refusal, actions()),
                "null".into(),
                "null".into(),
                "null".into(),
            ),
        };
        envelope.payload_json = format!(
            "{{\"schema\":\"fmine.plant-material-result.v1\",\"mode\":\"exact_nominal_material_projection\",\"source\":{{\"path\":{},\"sha256\":{},\"bytes\":{}}},\"scope\":{scope_json},\"observations\":[{}],\"admitted_claim\":{admitted},\"receipt_digest\":{digest},\"result\":{result}}}",
            quoted(&source.artifact_path),
            quoted(&Digest32(source.artifact_sha256).to_hex()),
            text.len(),
            observations
                .iter()
                .map(observation_json)
                .collect::<Vec<_>>()
                .join(","),
        );
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant project", refusal, actions()));
    envelope.no_claim = Some(measurements::NO_CLAIM);
    envelope.warnings.push("All worksheet values, total-assay interpretation and whole-material support are Assumed, Internal and uncalibrated; byte binding does not verify measurement accuracy or sample representativeness.".into());
    envelope
}
