//! DESIGN: project explicitly supported nominal masses and dry-basis total
//! elemental assays into extensive material quantities (BD-177/336).
//! Dry matter means all non-water material, including dissolved dry residue.
//! A separated-solids sample or a leachable/speciated assay is not such an input.
//! No sampling extrapolation, chemical conversion or error covariance is inferred.
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, UnitsError};

use crate::flowsheet::{ElementKey, MaterialAmounts};
use crate::inventory;

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);

pub const INPUT_METRIC: &str = "plant.measurement.input/1";
pub const OUTPUT_METRIC: &str = "plant.measurement.material/1";
pub const NO_CLAIM: &str = "Exact nominal projection conditional on masses and total-element assays covering the same full physical material and time. Dry matter includes all non-water residue. No sample-to-material extrapolation, chemical conversion, uncertainty propagation, reconciliation, recovery estimate or P754 compliance is established.";

/// Source-declared mass unit; tonnes here are metric tonnes, exactly 1000 kg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassUnit {
    Kilograms,
    Tonnes,
}

impl MassUnit {
    fn kilograms(self, value: ExactQty) -> Result<ExactQty> {
        match self {
            Self::Kilograms => Ok(value),
            Self::Tonnes => exact(value.checked_mul(ExactQty::from_integer(1000))),
        }
    }
}

/// Total elemental mass per mass of all non-water dry matter. These units do
/// not admit solution concentration, wet-basis grade or acid-soluble fractions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssayUnit {
    MassFraction,
    MassPercent,
    GramsPerTonne,
}

impl AssayUnit {
    fn mass_fraction(self, value: ExactQty) -> Result<ExactQty> {
        let denominator = match self {
            Self::MassFraction => 1,
            Self::MassPercent => 100,
            Self::GramsPerTonne => 1_000_000,
        };
        exact(value.checked_div(ExactQty::from_integer(denominator)))
    }
}

/// Shared full-material support, at the full physical boundary rather than an
/// ownership share. The same instant can describe a stocktake; a nonempty
/// interval describes an integrated transfer. No time averaging is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementScope {
    pub boundary: String,
    pub material: String,
    pub physical_time: ValidTime,
}

/// Exactly one mass route is admitted: DryMass + WaterMass, or WetMass +
/// Moisture. Each route also needs at least one uniquely identified DryAssay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasurementValue {
    DryMass {
        value: ExactQty,
        unit: MassUnit,
    },
    WetMass {
        value: ExactQty,
        unit: MassUnit,
    },
    /// H2O mass, not total solution mass. An explicit zero is required for dry material.
    WaterMass {
        value: ExactQty,
        unit: MassUnit,
    },
    /// H2O / total wet material mass, a fraction in [0, 1), not a percentage or
    /// the different water / dry-matter ratio.
    Moisture {
        water_over_wet_mass: ExactQty,
    },
    /// A source-declared total elemental assay of the whole material's dry
    /// matter, with exactly this measurand identity. Partial/speciated assays
    /// and separated-solid samples require a different explicit model.
    DryAssay {
        element: ElementKey,
        value: ExactQty,
        unit: AssayUnit,
    },
}

impl MeasurementValue {
    #[must_use]
    pub const fn additivity(&self) -> AdditivityClass {
        match self {
            Self::DryMass { .. } | Self::WetMass { .. } | Self::WaterMass { .. } => {
                AdditivityClass::ExtensiveAdditive
            }
            Self::Moisture { .. } | Self::DryAssay { .. } => AdditivityClass::IntensiveWeighted,
        }
    }

    fn key(&self) -> (u8, &str, &str) {
        match self {
            Self::DryMass { .. } => (0, "", ""),
            Self::WetMass { .. } => (1, "", ""),
            Self::WaterMass { .. } => (2, "", ""),
            Self::Moisture { .. } => (3, "", ""),
            Self::DryAssay { element, .. } => (4, &element.element, &element.measurand),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialMeasurement {
    pub scope: MeasurementScope,
    pub value: MeasurementValue,
}

impl MaterialMeasurement {
    /// Bind every unit, basis, material identity, time and exact source value.
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PLMI", 1);
        enc.put_str(&self.scope.boundary);
        enc.put_str(&self.scope.material);
        enc.put_i64(self.scope.physical_time.start_ns);
        enc.put_i64(self.scope.physical_time.end_ns);
        enc.put_str("full-physical-material/total-elements/total-non-water-dry-matter");
        enc.put_u8(self.value.key().0);
        match &self.value {
            MeasurementValue::DryMass { value, unit }
            | MeasurementValue::WetMass { value, unit }
            | MeasurementValue::WaterMass { value, unit } => {
                put_qty(&mut enc, *value);
                enc.put_u8(*unit as u8);
            }
            MeasurementValue::Moisture {
                water_over_wet_mass,
            } => put_qty(&mut enc, *water_over_wet_mass),
            MeasurementValue::DryAssay {
                element,
                value,
                unit,
            } => {
                enc.put_str(&element.element);
                enc.put_str(&element.measurand);
                put_qty(&mut enc, *value);
                enc.put_u8(*unit as u8);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Extensive kilograms on the unchanged declared support. Elements overlap
/// the dry carrier; their masses are not added to dry matter plus H2O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialProjection {
    pub scope: MeasurementScope,
    pub amounts: MaterialAmounts,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialProjectionReceipt {
    inputs: Vec<Claim<MaterialMeasurement>>,
    claim: Claim<MaterialProjection>,
    digest: String,
}

impl MaterialProjectionReceipt {
    pub fn inputs(&self) -> &[Claim<MaterialMeasurement>] {
        &self.inputs
    }
    pub fn summary(&self) -> &MaterialProjection {
        &self.claim.value
    }
    pub fn claim(&self) -> &Claim<MaterialProjection> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant measurement: {}", detail.into()))
}
fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact material projection is unrepresentable",
        )
    })
}
fn named(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && !text.chars().any(char::is_control)
}
fn put_qty(enc: &mut Encoder, value: ExactQty) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}
fn fraction(value: ExactQty, strict_upper: bool) -> Result<()> {
    let order = exact(value.checked_cmp(ONE))?;
    if value.signum() < 0 || order.is_gt() || (strict_upper && order.is_eq()) {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            if strict_upper {
                "wet-basis moisture must be a fraction in [0, 1)"
            } else {
                "dry-basis total-element mass fraction must be in [0, 1]"
            },
        ));
    }
    Ok(())
}

/// Convert nominal reported masses and total dry-basis assays without changing
/// their source support or inventing a measurement error model. H/O assays are
/// refused because their total elemental mass includes water, which would need
/// an explicitly versioned stoichiometric conversion. No other element's mass
/// is silently obtained from an oxide, sulfide or leachable-fraction assay.
pub fn project_material(
    inputs: &[Claim<MaterialMeasurement>],
) -> Result<MaterialProjectionReceipt> {
    if inputs.len() < 3 {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "one complete mass route and at least one total dry-basis assay are required",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.value.value.key().cmp(&b.value.value.key()));
    let scope = inputs[0].value.scope.clone();
    if !named(&scope.boundary) || !named(&scope.material) {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "a boundary version and full physical material identity are required",
        ));
    }
    if scope.physical_time.start_ns > scope.physical_time.end_ns {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "the material's physical interval is reversed",
        ));
    }

    let (mut dry, mut wet, mut water, mut moisture) = (None, None, None, None);
    let mut assays = BTreeMap::new();
    let mut elements = BTreeSet::new();
    let mut total_fraction = ZERO;
    for (i, input) in inputs.iter().enumerate() {
        inventory::admit_source(
            input,
            INPUT_METRIC,
            &input.value.canonical_tag(),
            input.value.value.additivity(),
        )?;
        if input.value.scope != scope {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                "masses, moisture and assays must cover the same boundary, material and physical time",
            ));
        }
        if i > 0 && input.value.value.key() == inputs[i - 1].value.value.key() {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate measurement role or elemental assay",
            ));
        }
        match &input.value.value {
            MeasurementValue::DryMass { value, unit }
            | MeasurementValue::WetMass { value, unit }
            | MeasurementValue::WaterMass { value, unit } => {
                if value.signum() < 0 {
                    return Err(bad(
                        RefusalCode::OutsidePhysicalDomain109,
                        "material masses must be nonnegative",
                    ));
                }
                let kg = unit.kilograms(*value)?;
                match &input.value.value {
                    MeasurementValue::DryMass { .. } => dry = Some(kg),
                    MeasurementValue::WetMass { .. } => wet = Some(kg),
                    _ => water = Some(kg),
                }
            }
            MeasurementValue::Moisture {
                water_over_wet_mass,
            } => {
                fraction(*water_over_wet_mass, true)?;
                moisture = Some(*water_over_wet_mass);
            }
            MeasurementValue::DryAssay {
                element,
                value,
                unit,
            } => {
                let symbol = element.element.as_bytes();
                if !named(&element.measurand)
                    || !(symbol.len() == 1 || symbol.len() == 2)
                    || !symbol[0].is_ascii_uppercase()
                    || (symbol.len() == 2 && !symbol[1].is_ascii_lowercase())
                {
                    return Err(bad(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        "assays need a case-sensitive elemental symbol and total-element measurand identity",
                    ));
                }
                if matches!(element.element.as_str(), "H" | "O") {
                    return Err(bad(
                        RefusalCode::BasisMismatch105,
                        "dry H/O assay does not supply total element in water; no implicit stoichiometry is admitted",
                    ));
                }
                if !elements.insert(element.element.clone()) {
                    return Err(bad(
                        RefusalCode::BasisMismatch105,
                        "the same element cannot have multiple competing assay measurands",
                    ));
                }
                let ratio = unit.mass_fraction(*value)?;
                fraction(ratio, false)?;
                total_fraction = exact(total_fraction.checked_add(ratio))?;
                assays.insert(element.clone(), ratio);
            }
        }
    }
    if assays.is_empty() {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "at least one total dry-basis elemental assay is required",
        ));
    }
    if exact(total_fraction.checked_cmp(ONE))?.is_gt() {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "distinct total-element assays sum to more than all dry matter",
        ));
    }
    let (dry_kg, water_kg) = match (dry, wet, water, moisture) {
        (Some(dry), None, Some(water), None) => (dry, water),
        (None, Some(wet), None, Some(moisture)) => (
            exact(wet.checked_mul(exact(ONE.checked_sub(moisture))?))?,
            exact(wet.checked_mul(moisture))?,
        ),
        _ => {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "declare exactly DryMass + WaterMass or WetMass + wet-basis Moisture; no missing water defaults to zero and no conflicting mass routes are reconciled",
            ));
        }
    };
    if dry_kg == ZERO && total_fraction != ZERO {
        return Err(bad(
            RefusalCode::NotObservable202,
            "a nonzero dry-basis assay cannot characterize absent dry matter",
        ));
    }
    let mut elements_kg = BTreeMap::new();
    for (element, ratio) in assays {
        elements_kg.insert(element, exact(dry_kg.checked_mul(ratio))?);
    }
    let summary = MaterialProjection {
        scope,
        amounts: MaterialAmounts {
            dry_solids_kg: dry_kg,
            water_kg,
            elements_kg,
        },
    };

    let mut enc = Encoder::new_frame(*b"PLMR", 1);
    enc.put_u64(inputs.len() as u64);
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut uu = BTreeMap::new();
    let mut parents = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    for input in &inputs {
        inventory::put_source_envelope(&mut enc, input, input.value.value.key().0);
        evidence = evidence.meet(input.evidence);
        epistemic = epistemic.and(input.epistemic);
        state = state.and(input.state);
        confidentiality = confidentiality.join(&input.confidentiality);
        parents.insert(input.claim_id);
        anchors.extend(input.anchors.iter().cloned());
        for term in &input.uu_terms {
            if let Some(previous) = uu.insert(term.scenario_id.clone(), term.clone())
                && previous != *term
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting definitions of an unknown-unknown scenario",
                ));
            }
        }
    }
    put_qty(&mut enc, dry_kg);
    put_qty(&mut enc, water_kg);
    enc.put_u64(summary.amounts.elements_kg.len() as u64);
    for (element, amount) in &summary.amounts.elements_kg {
        enc.put_str(&element.element);
        enc.put_str(&element.measurand);
        put_qty(&mut enc, *amount);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &digest,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    );
    Ok(MaterialProjectionReceipt {
        inputs,
        claim,
        digest,
    })
}

#[cfg(test)]
mod tests;
