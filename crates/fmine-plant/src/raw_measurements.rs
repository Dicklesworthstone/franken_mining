//! DESIGN: raw lot weights, bulk water fractions and dry elemental assays to
//! one extensive transfer (plan §§17.5, 20.1–20.3; BD-151/155/177).
//! Exact nominal products are summed per lot before any grade is derived.
//! The separately declared full correlation model is propagated to first order;
//! f(x) is not asserted to be E[f(X)], and no Gaussian output law is inferred.
//! Reference: NIST TN 1297, Appendix A.3, first-order propagation with covariance:
//! <https://www.nist.gov/pml/nist-technical-note-1297/nist-tn-1297-appendix-law-propagation-uncertainty>.
//! Determinism: DeterministicPerIsa (covariance); nominal arithmetic is Exact.

use std::collections::{BTreeMap, BTreeSet};

use crate::flowsheet::{
    ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, StreamId,
};
use crate::inventory::{admit_source_at_support, put_source_envelope};
use crate::reconciliation::{Kilograms, KilogramsSquared, Metadata};
use fmine_claim::{AdditivityClass, Claim, ClaimId, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::{MAX_OBSERVATIONS, propagate_linear_covariance};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, UnitsError};

pub mod worksheet;

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
pub const INPUT_METRIC: &str = "plant.raw_transfer.input/1";
pub const ERROR_MODEL_METRIC: &str = "plant.raw_transfer.linearized_error_model/1";
pub const OUTPUT_METRIC: &str = "plant.raw_transfer.projection/1";

/// Values are total kg and kg/kg fractions, never tonnes, percent or g/t.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawTransferValue {
    Declaration {
        lot_ids: Vec<String>,
        elements: Vec<ElementKey>,
        complete: bool,
        nonoverlapping_lots: bool,
        /// Drying loss alone does not prove this; the source must supply it.
        moisture_is_h2o: bool,
        /// Covers all non-water matter, including any dissolved non-water matter.
        assays_are_total_elemental: bool,
    },
    WetMass {
        lot: String,
        kg: ExactQty,
    },
    Moisture {
        lot: String,
        wet_mass_fraction: ExactQty,
    },
    DryAssay {
        lot: String,
        element: ElementKey,
        dry_mass_fraction: ExactQty,
    },
}
impl RawTransferValue {
    pub const fn additivity(&self) -> AdditivityClass {
        match self {
            Self::Declaration { .. } => AdditivityClass::NonAdditive,
            Self::WetMass { .. } => AdditivityClass::ExtensiveAdditive,
            _ => AdditivityClass::IntensiveWeighted,
        }
    }
    fn key(&self) -> (u8, &str, u8, &str, &str) {
        match self {
            Self::Declaration { .. } => (0, "", 0, "", ""),
            Self::WetMass { lot, .. } => (1, lot, 0, "", ""),
            Self::Moisture { lot, .. } => (1, lot, 1, "", ""),
            Self::DryAssay { lot, element, .. } => {
                (1, lot, 2, &element.element, &element.measurand)
            }
        }
    }
    fn amount(&self) -> Option<ExactQty> {
        match self {
            Self::Declaration { .. } => None,
            Self::WetMass { kg, .. } => Some(*kg),
            Self::Moisture {
                wet_mass_fraction, ..
            } => Some(*wet_mass_fraction),
            Self::DryAssay {
                dry_mass_fraction, ..
            } => Some(*dry_mass_fraction),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTransferInput {
    pub scope: FlowsheetScope,
    pub stream: StreamId,
    /// A named, matched bulk-moisture/assay dry-state basis, bound into every source.
    pub dry_basis: String,
    /// Declaration: full interval. All observations of one lot: identical support.
    pub physical_time: ValidTime,
    pub value: RawTransferValue,
}
impl RawTransferInput {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RLSI", 1);
        put_scope(&mut enc, &self.scope, &self.stream);
        enc.put_str(&self.dry_basis);
        enc.put_i64(self.physical_time.start_ns);
        enc.put_i64(self.physical_time.end_ns);
        let key = self.value.key();
        enc.put_u8(key.0);
        enc.put_str(key.1);
        enc.put_u8(key.2);
        enc.put_str(key.3);
        enc.put_str(key.4);
        if let RawTransferValue::Declaration {
            lot_ids,
            elements,
            complete,
            nonoverlapping_lots,
            moisture_is_h2o,
            assays_are_total_elemental,
        } = &self.value
        {
            for v in [
                complete,
                nonoverlapping_lots,
                moisture_is_h2o,
                assays_are_total_elemental,
            ] {
                enc.put_u8(u8::from(*v));
            }
            let mut lots = lot_ids.clone();
            lots.sort();
            enc.put_u64(lots.len() as u64);
            for lot in lots {
                enc.put_str(&lot);
            }
            let mut elements = elements.clone();
            elements.sort();
            enc.put_u64(elements.len() as u64);
            for element in elements {
                put_element(&mut enc, &element);
            }
        } else if let Some(amount) = self.value.amount() {
            put_qty(&mut enc, amount);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Dimensional scales for a dimensionless raw correlation matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StandardUncertainty {
    MassKg(Kilograms),
    Fraction(f64),
}
impl StandardUncertainty {
    fn scalar(self) -> f64 {
        match self {
            Self::MassKg(q) => q.value(),
            Self::Fraction(q) => q,
        }
    }
}

/// Full raw correlation, including across lots and between weight, water and assay.
/// No zero off-diagonal, common instrument independence or variance is inferred.
#[derive(Debug, Clone, PartialEq)]
pub struct RawTransferErrorModel {
    pub scope: FlowsheetScope,
    pub stream: StreamId,
    pub declaration_id: ClaimId,
    /// Canonical lot order; within a lot: wet mass, moisture, then sorted elements.
    pub observation_ids: Vec<ClaimId>,
    pub standard_uncertainties: Vec<StandardUncertainty>,
    /// Row-major n×n, strictly positive definite, exactly symmetric, unit diagonal.
    pub correlation: Vec<f64>,
    /// Source-backed applicability assumption, not a computed curvature guarantee.
    pub first_order_approximation_accepted: bool,
}
impl RawTransferErrorModel {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RLSE", 1);
        put_scope(&mut enc, &self.scope, &self.stream);
        enc.put_str(&self.declaration_id.to_string());
        enc.put_u64(self.observation_ids.len() as u64);
        for id in &self.observation_ids {
            enc.put_str(&id.to_string());
        }
        enc.put_u64(self.standard_uncertainties.len() as u64);
        for sigma in &self.standard_uncertainties {
            enc.put_u8(u8::from(matches!(sigma, StandardUncertainty::MassKg(_))));
            enc.put_u64(sigma.scalar().to_bits());
        }
        enc.put_u64(self.correlation.len() as u64);
        for value in &self.correlation {
            enc.put_u64(value.to_bits());
        }
        enc.put_u8(u8::from(self.first_order_approximation_accepted));
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedChannel {
    DryMatter,
    Water,
    Element(ElementKey),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferProjection {
    pub scope: FlowsheetScope,
    pub stream: StreamId,
    pub dry_basis: String,
    pub amounts: MaterialAmounts,
    /// Ratios of summed elemental mass to summed dry matter, never averages of grades.
    pub dry_grade_mass_fractions: BTreeMap<ElementKey, ExactQty>,
    pub covariance_order: Vec<ProjectedChannel>,
    /// J diag(sigma) R diag(sigma) J'; a first-order approximation in kg².
    /// This matrix alone supplies neither an unbiased Gaussian model nor coverage.
    pub linearized_covariance: Vec<KilogramsSquared>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferProjectionReceipt {
    inputs: Vec<Claim<RawTransferInput>>,
    error_model: Claim<RawTransferErrorModel>,
    claim: Claim<TransferProjection>,
    transfer: Claim<FlowsheetInput>,
    digest: String,
}
impl TransferProjectionReceipt {
    pub fn inputs(&self) -> &[Claim<RawTransferInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<RawTransferErrorModel> {
        &self.error_model
    }
    pub fn claim(&self) -> &Claim<TransferProjection> {
        &self.claim
    }
    /// Nominal extensive observation, retaining the full projection claim as a parent.
    /// Its error-model adoption for reconciliation requires an explicit declaration.
    pub fn transfer(&self) -> &Claim<FlowsheetInput> {
        &self.transfer
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn fail(code: RefusalCode, message: &str) -> Refusal {
    Refusal::new(code, format!("raw transfer projection: {message}"))
}
fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "exact nominal arithmetic overflow",
        )
    })
}
fn named(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && !s.chars().any(char::is_control)
}
fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}
fn put_element(enc: &mut Encoder, e: &ElementKey) {
    enc.put_str(&e.element);
    enc.put_str(&e.measurand);
}
fn put_scope(enc: &mut Encoder, scope: &FlowsheetScope, stream: &StreamId) {
    enc.put_str(&scope.boundary);
    enc.put_i64(scope.interval.start_ns);
    enc.put_i64(scope.interval.end_ns);
    enc.put_str(stream.as_str());
    enc.put_str("kg-wet-mass/wet-water-fraction/dry-total-element-mass-fraction");
}

struct Admitted<'a> {
    sources: Vec<&'a Claim<RawTransferInput>>,
    elements: Vec<ElementKey>,
    lots: Vec<String>,
}

fn admit(inputs: &[Claim<RawTransferInput>]) -> Result<Admitted<'_>> {
    if inputs.len() < 4 || inputs.len() > MAX_OBSERVATIONS + 1 {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "require a complete lot population within the 128-observation work bound",
        ));
    }
    let mut sources: Vec<_> = inputs.iter().collect();
    sources.sort_by(|a, b| a.value.value.key().cmp(&b.value.value.key()));
    let first = &sources[0].value;
    let RawTransferValue::Declaration {
        lot_ids,
        elements,
        complete: true,
        nonoverlapping_lots: true,
        moisture_is_h2o: true,
        assays_are_total_elemental: true,
    } = &first.value
    else {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare complete nonoverlapping lots, water-specific moisture and total elemental assays on all non-water matter",
        ));
    };
    let lots: BTreeSet<_> = lot_ids.iter().cloned().collect();
    let schema: BTreeSet<_> = elements.iter().cloned().collect();
    if lots.len() != lot_ids.len()
        || schema.len() != elements.len()
        || lots.is_empty()
        || schema.is_empty()
        || lots.iter().any(|s| !named(s))
        || schema.len() + 2 > MAX_OBSERVATIONS
        || lots.len().checked_mul(schema.len() + 2) != Some(inputs.len() - 1)
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "duplicate, empty or incomplete lot/element declaration",
        ));
    }
    let mut symbols = BTreeSet::new();
    for e in &schema {
        let bytes = e.element.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 2
            || !bytes[0].is_ascii_uppercase()
            || !bytes[1..].iter().all(u8::is_ascii_lowercase)
            || !named(&e.measurand)
            || !symbols.insert(&e.element)
            || matches!(e.element.as_str(), "H" | "O")
        {
            return Err(fail(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "declare distinct non-H/O elemental measurands; a dry assay cannot supply total water-borne H/O",
            ));
        }
    }
    if !named(&first.scope.boundary)
        || !named(&first.dry_basis)
        || first.scope.interval.start_ns >= first.scope.interval.end_ns
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "name a dry basis and physical boundary with positive accounting interval",
        ));
    }
    let elements: Vec<_> = schema.into_iter().collect();
    let lots: Vec<_> = lots.into_iter().collect();
    let width = elements.len() + 2;
    let mut ids = BTreeSet::new();
    for (i, input) in sources.iter().enumerate() {
        let value = &input.value;
        admit_source_at_support(
            input,
            INPUT_METRIC,
            &value.canonical_tag(),
            value.value.additivity(),
            if i == 0 {
                SpatialSupport::PeriodAggregate
            } else {
                SpatialSupport::PlantLot
            },
        )?;
        if value.scope != first.scope
            || value.stream != first.stream
            || value.dry_basis != first.dry_basis
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "mixed stream, boundary, interval or dry-state basis",
            ));
        }
        if !ids.insert(input.claim_id) {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "repeated source claim identity",
            ));
        }
        if i == 0 {
            if value.physical_time != first.scope.interval {
                return Err(fail(
                    RefusalCode::SupportMismatch102,
                    "population declaration must cover the full transfer interval",
                ));
            }
            continue;
        }
        let lot_index = (i - 1) / width;
        let column = (i - 1) % width;
        let key = value.value.key();
        let expected_kind = if column < 2 { column as u8 } else { 2 };
        if key.0 != 1
            || key.1 != lots[lot_index]
            || key.2 != expected_kind
            || (column >= 2
                && (key.3 != elements[column - 2].element
                    || key.4 != elements[column - 2].measurand))
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "require one wet mass, one moisture and every declared assay per lot",
            ));
        }
        let t = value.physical_time;
        if t.start_ns < first.scope.interval.start_ns
            || t.end_ns > first.scope.interval.end_ns
            || t.end_ns < t.start_ns
            || t.start_ns == first.scope.interval.end_ns
            || t != sources[1 + lot_index * width].value.physical_time
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "all observations of a lot need identical physical support within the reporting interval",
            ));
        }
        let amount = value
            .value
            .amount()
            .ok_or_else(|| fail(RefusalCode::BasisMismatch105, "unexpected declaration"))?;
        if amount.signum() < 0
            || (column == 0 && amount.signum() == 0)
            || (column == 1 && exact(amount.checked_cmp(ONE))?.is_ge())
            || (column >= 2 && exact(amount.checked_cmp(ONE))?.is_gt())
        {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "require positive wet kg, water fraction in [0,1), and dry elemental fractions in [0,1]",
            ));
        }
    }
    for lot in sources[1..].chunks_exact(width) {
        let mut total = ZERO;
        for assay in &lot[2..] {
            total = exact(total.checked_add(assay.value.value.amount().ok_or_else(|| {
                fail(
                    RefusalCode::BasisMismatch105,
                    "missing admitted elemental assay",
                )
            })?))?;
        }
        if exact(total.checked_cmp(ONE))?.is_gt() {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "distinct elemental fractions exceed all non-water dry matter in a lot",
            ));
        }
    }
    Ok(Admitted {
        sources,
        elements,
        lots,
    })
}

/// Canonical IDs for an explicit raw correlation model, after source admission.
pub fn observation_order(inputs: &[Claim<RawTransferInput>]) -> Result<Vec<ClaimId>> {
    Ok(admit(inputs)?.sources[1..]
        .iter()
        .map(|x| x.claim_id)
        .collect())
}

fn admit_error_model(inputs: &Admitted<'_>, source: &Claim<RawTransferErrorModel>) -> Result<()> {
    let model = &source.value;
    let declaration = inputs.sources[0];
    let n = inputs.sources.len() - 1;
    admit_source_at_support(
        source,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
        SpatialSupport::PeriodAggregate,
    )?;
    if model.scope != declaration.value.scope
        || model.stream != declaration.value.stream
        || model.declaration_id != declaration.claim_id
        || !model
            .observation_ids
            .iter()
            .copied()
            .eq(inputs.sources[1..].iter().map(|s| s.claim_id))
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "error model must bind this declaration, stream, scope and every canonical raw observation",
        ));
    }
    if model.standard_uncertainties.len() != n || model.correlation.len() != n * n {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "supply one dimensional uncertainty per raw coordinate and a complete correlation matrix",
        ));
    }
    if !model.first_order_approximation_accepted {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "source must explicitly accept the first-order approximation for these measurements",
        ));
    }
    for (input, uncertainty) in inputs.sources[1..]
        .iter()
        .zip(&model.standard_uncertainties)
    {
        if matches!(input.value.value, RawTransferValue::WetMass { .. })
            != matches!(uncertainty, StandardUncertainty::MassKg(_))
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "weight uncertainty must be kg; moisture and dry-assay uncertainties must be fractions",
            ));
        }
        if !uncertainty.scalar().is_finite() || uncertainty.scalar() < 0.0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "standard uncertainties must be finite and nonnegative; zero must be explicit",
            ));
        }
    }
    if model
        .correlation
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1.0)
        || (0..n).any(|i| model.correlation[i * n + i] != 1.0)
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "correlation must be finite, within [-1,1], and have exactly unit diagonal",
        ));
    }
    // Exact symmetry and positive definiteness belong to the shared numerical owner.
    Ok(())
}

// Form the exact derivative before binary64 conversion. In particular, 1-w
// must not be evaluated after w rounds to 1.0. Nonzero propagation may not
// silently disappear when multiplied by a very small standard uncertainty.
fn standardized_derivative(derivative: ExactQty, uncertainty: StandardUncertainty) -> Result<f64> {
    let sigma = uncertainty.scalar();
    if derivative == ZERO || sigma == 0.0 {
        return Ok(0.0);
    }
    let represented = derivative.to_f64();
    let scaled = represented * sigma;
    if !represented.is_finite() || represented == 0.0 || !scaled.is_finite() || scaled == 0.0 {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "nonzero standardized derivative loses numeric range",
        ));
    }
    Ok(scaled)
}

/// Project one stream's explicitly complete, nonoverlapping lot population.
/// Each lot supplies wet kg, water/wet-mass fraction and every total-element
/// dry-mass fraction on the same named dry state. Exact nominal products are
/// added before deriving the aggregate grade. No water or independence defaults
/// are supplied. The covariance is only the declared first-order approximation;
/// no unbiased Gaussian transformed law, sampling coverage or WLS error model
/// follows from this calculation. Perfect raw correlations need a separate
/// lower-dimensional error declaration; no rank repair or variance is added.
pub fn project_transfer(
    inputs: &[Claim<RawTransferInput>],
    error_model: &Claim<RawTransferErrorModel>,
) -> Result<TransferProjectionReceipt> {
    let admitted = admit(inputs)?;
    admit_error_model(&admitted, error_model)?;
    let n = admitted.sources.len() - 1;
    let width = admitted.elements.len() + 2;
    let mut jacobian = vec![0.0; width * n];
    let mut dry_total = ZERO;
    let mut water_total = ZERO;
    let mut element_totals = vec![ZERO; admitted.elements.len()];
    let scales = &error_model.value.standard_uncertainties;
    for lot_index in 0..admitted.lots.len() {
        let start = lot_index * width;
        let lot = &admitted.sources[1 + start..1 + start + width];
        let values = lot
            .iter()
            .map(|s| {
                s.value.value.amount().ok_or_else(|| {
                    fail(RefusalCode::BasisMismatch105, "missing admitted raw amount")
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mass = values[0];
        let moisture = values[1];
        let dry_fraction = exact(ONE.checked_sub(moisture))?;
        let dry = exact(mass.checked_mul(dry_fraction))?;
        let water = exact(mass.checked_mul(moisture))?;
        dry_total = exact(dry_total.checked_add(dry))?;
        water_total = exact(water_total.checked_add(water))?;

        // Rows: dry matter, H2O, then sorted elements. Columns: all lots in
        // canonical order, each with mass, moisture, then sorted dry assays.
        jacobian[start] = standardized_derivative(dry_fraction, scales[start])?;
        jacobian[start + 1] =
            standardized_derivative(exact(mass.checked_neg())?, scales[start + 1])?;
        jacobian[n + start] = standardized_derivative(moisture, scales[start])?;
        jacobian[n + start + 1] = standardized_derivative(mass, scales[start + 1])?;
        for (e, total) in element_totals.iter_mut().enumerate() {
            let grade = values[e + 2];
            *total = exact(total.checked_add(exact(dry.checked_mul(grade))?))?;
            let row = (e + 2) * n + start;
            jacobian[row] =
                standardized_derivative(exact(dry_fraction.checked_mul(grade))?, scales[start])?;
            jacobian[row + 1] = standardized_derivative(
                exact(exact(mass.checked_mul(grade))?.checked_neg())?,
                scales[start + 1],
            )?;
            jacobian[row + e + 2] = standardized_derivative(dry, scales[start + e + 2])?;
        }
    }
    let linearized_covariance =
        propagate_linear_covariance(&error_model.value.correlation, &jacobian, n, width)?
            .into_iter()
            .map(KilogramsSquared::new)
            .collect();
    let mut grades = BTreeMap::new();
    let mut elements_kg = BTreeMap::new();
    let mut covariance_order = vec![ProjectedChannel::DryMatter, ProjectedChannel::Water];
    for (element, mass) in admitted.elements.iter().zip(element_totals) {
        grades.insert(element.clone(), exact(mass.checked_div(dry_total))?);
        elements_kg.insert(element.clone(), mass);
        covariance_order.push(ProjectedChannel::Element(element.clone()));
    }
    let declaration = &admitted.sources[0].value;
    let projection = TransferProjection {
        scope: declaration.scope.clone(),
        stream: declaration.stream.clone(),
        dry_basis: declaration.dry_basis.clone(),
        amounts: MaterialAmounts {
            dry_solids_kg: dry_total,
            water_kg: water_total,
            elements_kg,
        },
        dry_grade_mass_fractions: grades,
        covariance_order,
        linearized_covariance,
    };
    let mut meta = Metadata::new();
    let mut enc = Encoder::new_frame(*b"RLSP", 1);
    enc.put_str("exact-nominal/first-order-covariance/no-transformed-distribution");
    enc.put_u64(admitted.sources.len() as u64);
    for source in &admitted.sources {
        let key = source.value.value.key();
        put_source_envelope(&mut enc, *source, if key.0 == 0 { 0 } else { key.2 + 1 });
        meta.include(*source)?;
    }
    put_source_envelope(&mut enc, error_model, 4);
    meta.include(error_model)?;
    put_qty(&mut enc, dry_total);
    put_qty(&mut enc, water_total);
    enc.put_u64(projection.amounts.elements_kg.len() as u64);
    for (element, mass) in &projection.amounts.elements_kg {
        put_element(&mut enc, element);
        put_qty(&mut enc, *mass);
        put_qty(&mut enc, projection.dry_grade_mass_fractions[element]);
    }
    enc.put_u64(projection.linearized_covariance.len() as u64);
    for value in &projection.linearized_covariance {
        enc.put_u64(value.value().to_bits());
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = meta.into_claim(OUTPUT_METRIC, projection, &digest);
    let transfer_input = FlowsheetInput {
        scope: claim.value.scope.clone(),
        physical_time: claim.value.scope.interval,
        value: FlowsheetValue::Transfer {
            stream: claim.value.stream.clone(),
            amounts: claim.value.amounts.clone(),
        },
    };
    let transfer_tag = transfer_input.canonical_tag();
    let transfer = Claim::new(
        crate::flowsheet::INPUT_METRIC,
        transfer_input,
        &transfer_tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        claim.evidence,
        claim.epistemic,
        claim.state,
        claim.confidentiality.clone(),
        claim.uu_terms.clone(),
        vec![claim.claim_id],
        claim.anchors.clone(),
    );
    Ok(TransferProjectionReceipt {
        inputs: admitted.sources.into_iter().cloned().collect(),
        error_model: error_model.clone(),
        claim,
        transfer,
        digest,
    })
}

#[cfg(test)]
mod tests;
