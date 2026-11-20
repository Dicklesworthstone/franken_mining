//! DESIGN: joint processing production from admitted grinding and recovery
//! populations (plan §17.4; BD-176/181/182). Multiply matched realizations before
//! averaging; scenario probabilities are never physical blending weights.
//! Determinism: DeterministicPerIsa

use std::collections::BTreeMap;

use crate::cards::{self, InputUnit, RecoveryScope, ensemble::RecoveryEnsembleReceipt};
use crate::grinding::{Distribution, DryTonnesPerHour, GrindingReceipt, GrindingScope};
use crate::inventory;
use crate::reconciliation::{Kilograms, Metadata};
use fmine_claim::{AdditivityClass, Claim, ClaimId, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_units::{Dry, ExactQty, Qty, UnitsError};

mod render;
#[cfg(test)]
mod tests;
pub mod worksheet;

pub const INPUT_METRIC: &str = "plant.production.joint_inputs/1";
pub const OUTPUT_METRIC: &str = "plant.production.joint_output/1";
pub const SUPPLY_METRIC: &str = "plant.production.available_feed/1";
pub const SUPPLY_OUTPUT_METRIC: &str = "plant.production.supply_limited_output/1";
pub const NO_CLAIM: &str = "Conditional production over the complete declared shared scenarios. Grinding throughput already includes its declared utilization; scheduled hours are elapsed exposure and do not apply utilization again. Grade and product-size coordinates are explicitly linked to recovery inputs. The declared coupling basis connects the grinding blend to the recovery material domain and dry state. No transport lag, stock movement, feedback, physical blending, plant validation, fitted uncertainty, calibrated probabilities, forecast registration, payable metal, financial value or operating authorization is inferred.";
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessedDryTonnes;
pub type DryTonnes = Qty<ProcessedDryTonnes, Dry>;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElapsedHours;
pub type Hours = Qty<ElapsedHours>;

/// A complete joint state of the same blend and interval. Hours denote
/// scheduled elapsed exposure: Bond throughput already includes utilization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionScenario {
    pub id: String,
    pub weight: ExactQty,
    pub grade_g_per_dry_t: ExactQty,
    pub scheduled_hours: ExactQty,
}

/// Source declaration connecting the differently named blend/domain supports.
/// Both recovery coordinates are explicit; no coordinate is guessed by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionInputs {
    pub grinding_scope: GrindingScope,
    pub recovery_scope: RecoveryScope,
    pub coupling_basis: String,
    pub grade_coordinate: String,
    pub product_size_coordinate: String,
    pub scenarios: Vec<ProductionScenario>,
}

impl ProductionInputs {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"JPIN", 1);
        for text in [
            &self.grinding_scope.boundary,
            &self.grinding_scope.circuit,
            &self.grinding_scope.blend,
            &self.recovery_scope.circuit,
            &self.recovery_scope.material_domain,
            &self.recovery_scope.dry_basis,
            &self.recovery_scope.element,
            &self.coupling_basis,
            &self.grade_coordinate,
            &self.product_size_coordinate,
        ] {
            enc.put_str(text);
        }
        enc.put_i64(self.grinding_scope.interval.start_ns);
        enc.put_i64(self.grinding_scope.interval.end_ns);
        let mut rows: Vec<_> = self.scenarios.iter().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        enc.put_u64(rows.len() as u64);
        for row in rows {
            enc.put_str(&row.id);
            for value in [row.weight, row.grade_g_per_dry_t, row.scheduled_hours] {
                put_exact(&mut enc, value);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Complete available feed for one shared state, in dry metric tonnes of the
/// same blend. Probability is a scenario weight, not a physical blending weight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplyScenario {
    pub id: String,
    pub weight: ExactQty,
    pub available_dry_tonnes: ExactQty,
}

/// DESIGN: finite feed is already present when processing starts. Curtailment
/// shortens exposure at the declared operating point; it does not throttle the
/// circuit or infer a changed recovery, utilization, grade or particle size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionSupply {
    pub grinding_scope: GrindingScope,
    pub recovery_scope: RecoveryScope,
    pub supply_basis: String,
    pub available_at_start: bool,
    pub unchanged_operating_point: bool,
    pub scenarios: Vec<SupplyScenario>,
}
impl ProductionSupply {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"JPSU", 1);
        for value in [
            &self.grinding_scope.boundary,
            &self.grinding_scope.circuit,
            &self.grinding_scope.blend,
            &self.recovery_scope.circuit,
            &self.recovery_scope.material_domain,
            &self.recovery_scope.dry_basis,
            &self.recovery_scope.element,
            &self.supply_basis,
        ] {
            enc.put_str(value);
        }
        enc.put_i64(self.grinding_scope.interval.start_ns);
        enc.put_i64(self.grinding_scope.interval.end_ns);
        enc.put_u8(u8::from(self.available_at_start));
        enc.put_u8(u8::from(self.unchanged_operating_point));
        let mut rows: Vec<_> = self.scenarios.iter().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        enc.put_u64(rows.len() as u64);
        for row in rows {
            enc.put_str(&row.id);
            put_exact(&mut enc, row.weight);
            put_exact(&mut enc, row.available_dry_tonnes);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingConstraint {
    OreSupply,
    GrindingCapacity,
    Both,
}
impl BindingConstraint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OreSupply => "ore_supply",
            Self::GrindingCapacity => "grinding_capacity",
            Self::Both => "both",
        }
    }
}

/// Conditional use of the declared available feed. Throughput on ProductionPoint
/// remains operating capacity; effective throughput divides processed tonnes by
/// scheduled exposure. Zero exposure defines both effective rate and use as zero.
#[derive(Debug, Clone, PartialEq)]
pub struct SupplyUse {
    pub available_dry_tonnes: ExactQty,
    pub capacity_dry_tonnes: DryTonnes,
    pub unused_dry_tonnes: DryTonnes,
    pub unused_capacity_dry_tonnes: DryTonnes,
    pub used_scheduled_hours: Hours,
    pub effective_throughput: DryTonnesPerHour,
    pub binding_constraint: BindingConstraint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionPoint {
    pub scenario_id: String,
    pub weight: ExactQty,
    pub grade_g_per_dry_t: ExactQty,
    pub scheduled_hours: ExactQty,
    pub grinding_source: ClaimId,
    pub recovery_source: ClaimId,
    pub throughput: DryTonnesPerHour,
    pub recovery_fraction: cards::Fraction,
    pub dry_tonnes: DryTonnes,
    pub contained_metal: Kilograms,
    pub recovered_metal: Kilograms,
    pub supply_use: Option<SupplyUse>,
}

/// Finite-population summaries with exact-weight inverse-CDF quantiles.
/// Masses are kg of the declared element, separately contained and recovered.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductionSummary {
    pub grinding_scope: GrindingScope,
    pub recovery_scope: RecoveryScope,
    pub scenario_count: usize,
    pub throughput: Distribution<DryTonnesPerHour>,
    pub recovery: Distribution<cards::Fraction>,
    pub dry_tonnes: Distribution<DryTonnes>,
    pub contained_metal: Distribution<Kilograms>,
    pub recovered_metal: Distribution<Kilograms>,
}

/// Immutable inputs, original model receipts and the complete joined result.
/// The Claim's scalar representation describes recovered metal in kg only.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductionReceipt {
    grinding: GrindingReceipt,
    recovery: RecoveryEnsembleReceipt,
    inputs: Claim<ProductionInputs>,
    supply: Option<Claim<ProductionSupply>>,
    points: Vec<ProductionPoint>,
    claim: Claim<ProductionSummary>,
    digest: String,
}
impl ProductionReceipt {
    pub fn grinding(&self) -> &GrindingReceipt {
        &self.grinding
    }
    pub fn recovery(&self) -> &RecoveryEnsembleReceipt {
        &self.recovery
    }
    pub fn inputs(&self) -> &Claim<ProductionInputs> {
        &self.inputs
    }
    pub fn supply(&self) -> Option<&Claim<ProductionSupply>> {
        self.supply.as_ref()
    }
    pub fn points(&self) -> &[ProductionPoint] {
        &self.points
    }
    pub fn claim(&self) -> &Claim<ProductionSummary> {
        &self.claim
    }
    pub fn summary(&self) -> &ProductionSummary {
        &self.claim.value
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn fail(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("joint plant production: {}", detail.into()))
}
fn numerical(detail: &str) -> Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, detail)
}
fn arithmetic(_: UnitsError) -> Refusal {
    numerical("exact input or probability arithmetic overflows")
}
fn named(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max
        && !value.chars().any(char::is_control)
}
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(numerical("nonfinite production arithmetic"))
    }
}
fn numeric(value: ExactQty) -> Result<f64> {
    let result = finite(value.to_f64())?;
    if result == 0.0 && value.signum() != 0 {
        return Err(numerical("nonzero exact input loses numerical range"));
    }
    Ok(result)
}
fn product(a: f64, b: f64) -> Result<f64> {
    let result = finite(a * b)?;
    if a != 0.0 && b != 0.0 && result == 0.0 {
        return Err(numerical("nonzero production product underflows"));
    }
    Ok(result)
}
fn put_exact(enc: &mut Encoder, value: ExactQty) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}
fn put_number(enc: &mut Encoder, value: f64) {
    enc.put_u64(if value == 0.0 { 0 } else { value.to_bits() });
}

fn admit_supply<'a>(
    inputs: &ProductionInputs,
    source: &'a Claim<ProductionSupply>,
) -> Result<BTreeMap<&'a str, &'a SupplyScenario>> {
    let value = &source.value;
    inventory::admit_source(
        source,
        SUPPLY_METRIC,
        &value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if value.grinding_scope != inputs.grinding_scope
        || value.recovery_scope != inputs.recovery_scope
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "available feed must name the same blend, dry state, circuit and interval",
        ));
    }
    if !named(&value.supply_basis, 4096)
        || !value.available_at_start
        || !value.unchanged_operating_point
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare feed available at processing start and curtail exposure at the unchanged operating point",
        ));
    }
    let mut rows = BTreeMap::new();
    if value.scenarios.len() != inputs.scenarios.len() {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "available feed must cover the complete shared scenario population",
        ));
    }
    for row in &value.scenarios {
        if !named(&row.id, 256) || rows.insert(row.id.as_str(), row).is_some() {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "available-feed scenario IDs must be unique bounded names",
            ));
        }
        if row.available_dry_tonnes.signum() < 0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "available dry tonnes must be nonnegative",
            ));
        }
    }
    Ok(rows)
}

fn supply_use(
    available: ExactQty,
    capacity: f64,
    throughput: f64,
    scheduled_hours: ExactQty,
) -> Result<(f64, SupplyUse)> {
    let available_value = numeric(available)?;
    let ordering = available
        .checked_cmp(crate::reconciliation::represented_rational(capacity)?)
        .map_err(arithmetic)?;
    if ordering != available_value.total_cmp(&capacity) {
        return Err(numerical(
            "binary64 conversion changes the exact supply/capacity ordering",
        ));
    }
    let (tonnes, binding_constraint) = match ordering {
        std::cmp::Ordering::Less => (available_value, BindingConstraint::OreSupply),
        std::cmp::Ordering::Equal => (capacity, BindingConstraint::Both),
        std::cmp::Ordering::Greater => (capacity, BindingConstraint::GrindingCapacity),
    };
    let hours = numeric(scheduled_hours)?;
    let (used_hours, effective_throughput) = if tonnes == 0.0 {
        (0.0, 0.0)
    } else if ordering.is_lt() {
        let used = finite(tonnes / throughput)?;
        let effective = finite(tonnes / hours)?;
        if used == 0.0 || effective == 0.0 || used > hours || effective > throughput {
            return Err(numerical(
                "curtailed exposure or effective rate loses range",
            ));
        }
        (used, effective)
    } else {
        (hours, throughput)
    };
    Ok((
        tonnes,
        SupplyUse {
            available_dry_tonnes: available,
            capacity_dry_tonnes: DryTonnes::new(capacity),
            unused_dry_tonnes: DryTonnes::new(finite(available_value - tonnes)?),
            unused_capacity_dry_tonnes: DryTonnes::new(finite(capacity - tonnes)?),
            used_scheduled_hours: Hours::new(used_hours),
            effective_throughput: DryTonnesPerHour::new(effective_throughput),
            binding_constraint,
        },
    ))
}

/// Join all three populations by scenario identity and exact probability.
/// Grade matches the recovery coordinate exactly; grind size matches at the
/// admitted Bond binary64 precision, without a tolerance or unit conversion.
pub fn evaluate(
    grinding: &GrindingReceipt,
    recovery: &RecoveryEnsembleReceipt,
    inputs: &Claim<ProductionInputs>,
) -> Result<ProductionReceipt> {
    evaluate_inner(grinding, recovery, inputs, None)
}

/// Process the minimum of explicitly available feed and grinding capacity over
/// the scheduled exposure, independently in every shared state. No arrival curve,
/// stock carryover, new operating point or recovery adjustment is inferred.
pub fn evaluate_supply_limited(
    grinding: &GrindingReceipt,
    recovery: &RecoveryEnsembleReceipt,
    inputs: &Claim<ProductionInputs>,
    supply: &Claim<ProductionSupply>,
) -> Result<ProductionReceipt> {
    evaluate_inner(grinding, recovery, inputs, Some(supply))
}

fn evaluate_inner(
    grinding: &GrindingReceipt,
    recovery: &RecoveryEnsembleReceipt,
    inputs: &Claim<ProductionInputs>,
    supply: Option<&Claim<ProductionSupply>>,
) -> Result<ProductionReceipt> {
    let value = &inputs.value;
    let count = value.scenarios.len();
    if count == 0 || count > cards::ensemble::MAX_SCENARIOS {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "requires 1..256 complete production scenarios",
        ));
    }
    inventory::admit_source(
        inputs,
        INPUT_METRIC,
        &value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let recovered = &recovery.claim().value;
    if value.grinding_scope != grinding.summary().scope
        || value.recovery_scope != recovered.scope
        || value.grinding_scope.circuit != value.recovery_scope.circuit
        || value.grinding_scope.interval != recovered.interval
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "declared scopes, common circuit and physical interval must match both receipts",
        ));
    }
    if !named(&value.coupling_basis, 4096)
        || !named(&value.grade_coordinate, 64)
        || !named(&value.product_size_coordinate, 64)
        || value.grade_coordinate == value.product_size_coordinate
    {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare a bounded material coupling basis and distinct grade and product-size coordinates",
        ));
    }
    if count != grinding.points().len() || count != recovery.members().len() {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "complete grinding, recovery and production populations must have the same members",
        ));
    }
    let supply_rows = supply
        .map(|source| admit_supply(value, source))
        .transpose()?;
    let interval = value.grinding_scope.interval;
    let interval_hours = ExactQty::new(
        i128::from(interval.end_ns) - i128::from(interval.start_ns),
        3_600_000_000_000,
    )
    .map_err(arithmetic)?;
    let grinding_inputs: BTreeMap<_, _> = grinding
        .scenarios()
        .iter()
        .map(|row| (row.value.id.as_str(), row))
        .collect();
    let grinding_points: BTreeMap<_, _> = grinding
        .points()
        .iter()
        .map(|row| (row.scenario_id.as_str(), row))
        .collect();
    let recovery_points: BTreeMap<_, _> = recovery
        .members()
        .iter()
        .map(|row| (row.id(), row))
        .collect();
    let mut rows: Vec<_> = value.scenarios.iter().collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    let mut total = ZERO;
    let mut points = Vec::with_capacity(count);
    for (index, row) in rows.iter().enumerate() {
        if !named(&row.id, 256) || (index > 0 && rows[index - 1].id == row.id) {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "production scenario IDs must be unique bounded names",
            ));
        }
        let missing = || {
            fail(
                RefusalCode::BasisMismatch105,
                format!("scenario {} is absent from a source population", row.id),
            )
        };
        let gp = grinding_points.get(row.id.as_str()).ok_or_else(missing)?;
        let gi = grinding_inputs.get(row.id.as_str()).ok_or_else(missing)?;
        let rp = recovery_points.get(row.id.as_str()).ok_or_else(missing)?;
        if row.weight != gp.weight || row.weight != rp.weight() {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                format!("scenario {} has different exact probabilities", row.id),
            ));
        }
        total = total.checked_add(row.weight).map_err(arithmetic)?;
        if row.grade_g_per_dry_t.signum() < 0
            || row
                .grade_g_per_dry_t
                .checked_cmp(ExactQty::from_integer(1_000_000))
                .map_err(arithmetic)?
                .is_gt()
            || row.scheduled_hours.signum() < 0
            || row
                .scheduled_hours
                .checked_cmp(interval_hours)
                .map_err(arithmetic)?
                .is_gt()
        {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "scenario {} needs grade in [0,1000000] g/dry t and hours within its physical interval",
                    row.id
                ),
            ));
        }
        let coordinates = &rp.receipt().inputs().value.values;
        let grade = coordinates.get(&value.grade_coordinate).ok_or_else(|| {
            fail(
                RefusalCode::NoClaimInDomain201,
                "linked recovery grade coordinate is missing",
            )
        })?;
        let grind = coordinates
            .get(&value.product_size_coordinate)
            .ok_or_else(|| {
                fail(
                    RefusalCode::NoClaimInDomain201,
                    "linked recovery product-size coordinate is missing",
                )
            })?;
        if grade.unit != InputUnit::GramsPerDryMetricTonne || grind.unit != InputUnit::Micrometres {
            return Err(fail(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "linked grade must use g/dry metric tonne and product size must use micrometres",
            ));
        }
        if grade.value != row.grade_g_per_dry_t
            || numeric(grind.value)? != gi.value.product_size.value()
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                format!("scenario {} has inconsistent grade or grind inputs", row.id),
            ));
        }
        let capacity = product(gp.throughput.value(), numeric(row.scheduled_hours)?)?;
        let (tonnes, supply_use) = if let Some(supply_rows) = &supply_rows {
            let available = supply_rows.get(row.id.as_str()).ok_or_else(missing)?;
            if available.weight != row.weight {
                return Err(fail(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "scenario {} has a different available-feed probability",
                        row.id
                    ),
                ));
            }
            let (tonnes, use_) = supply_use(
                available.available_dry_tonnes,
                capacity,
                gp.throughput.value(),
                row.scheduled_hours,
            )?;
            (tonnes, Some(use_))
        } else {
            (capacity, None)
        };
        let kg_per_tonne = row
            .grade_g_per_dry_t
            .checked_div(ExactQty::from_integer(1000))
            .map_err(arithmetic)?;
        let contained = product(tonnes, numeric(kg_per_tonne)?)?;
        let recovery_fraction = rp.receipt().claim().value.recovery_fraction;
        let recovered_kg = product(contained, recovery_fraction.value())?;
        points.push(ProductionPoint {
            scenario_id: row.id.clone(),
            weight: row.weight,
            grade_g_per_dry_t: row.grade_g_per_dry_t,
            scheduled_hours: row.scheduled_hours,
            grinding_source: gp.source,
            recovery_source: rp.receipt().claim().claim_id,
            throughput: gp.throughput,
            recovery_fraction,
            dry_tonnes: DryTonnes::new(tonnes),
            contained_metal: Kilograms::new(contained),
            recovered_metal: Kilograms::new(recovered_kg),
            supply_use,
        });
    }
    if total != ONE {
        return Err(numerical("joined exact probability mass is not one"));
    }
    let summary = ProductionSummary {
        grinding_scope: value.grinding_scope.clone(),
        recovery_scope: value.recovery_scope.clone(),
        scenario_count: count,
        throughput: grinding.summary().throughput.clone(),
        recovery: Distribution {
            mean: recovered.mean,
            minimum: recovered.minimum,
            p10: recovered.p10,
            p50: recovered.p50,
            p90: recovered.p90,
            maximum: recovered.maximum,
        },
        dry_tonnes: statistics(&points, |p| p.dry_tonnes)?,
        contained_metal: statistics(&points, |p| p.contained_metal)?,
        recovered_metal: statistics(&points, |p| p.recovered_metal)?,
    };
    let mut metadata = Metadata::new();
    metadata.evidence = EvidenceClass::ModelledUncalibrated;
    let mut enc = Encoder::new_frame(*b"JPRD", if supply.is_some() { 2 } else { 1 });
    for source in [grinding.digest(), recovery.digest()] {
        enc.put_str(source);
    }
    metadata.include(grinding.claim())?;
    metadata.include(recovery.claim())?;
    metadata.include(inputs)?;
    inventory::put_source_envelope(&mut enc, grinding.claim(), 0);
    inventory::put_source_envelope(&mut enc, recovery.claim(), 1);
    inventory::put_source_envelope(&mut enc, inputs, 2);
    if let Some(supply) = supply {
        metadata.include(supply)?;
        inventory::put_source_envelope(&mut enc, supply, 3);
    }
    for point in &points {
        enc.put_str(&point.scenario_id);
        put_exact(&mut enc, point.weight);
        for number in [
            point.dry_tonnes.value(),
            point.contained_metal.value(),
            point.recovered_metal.value(),
        ] {
            put_number(&mut enc, number);
        }
        if let Some(use_) = &point.supply_use {
            put_exact(&mut enc, use_.available_dry_tonnes);
            for number in [
                use_.capacity_dry_tonnes.value(),
                use_.unused_dry_tonnes.value(),
                use_.unused_capacity_dry_tonnes.value(),
                use_.used_scheduled_hours.value(),
                use_.effective_throughput.value(),
            ] {
                put_number(&mut enc, number);
            }
            enc.put_str(use_.binding_constraint.as_str());
        }
    }
    put_distribution(&mut enc, &summary.dry_tonnes);
    put_distribution(&mut enc, &summary.contained_metal);
    put_distribution(&mut enc, &summary.recovered_metal);
    let digest = blake3_hash(&enc.finish()).to_hex();
    let representation = Representation::EnsembleSummary {
        n: count as u32,
        mean: summary.recovered_metal.mean.value(),
        p10: summary.recovered_metal.p10.value(),
        p50: summary.recovered_metal.p50.value(),
        p90: summary.recovered_metal.p90.value(),
    };
    let claim = Claim::new(
        if supply.is_some() {
            SUPPLY_OUTPUT_METRIC
        } else {
            OUTPUT_METRIC
        },
        summary,
        &digest,
        representation,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        metadata.evidence,
        metadata.epistemic,
        metadata.state.and(KnowledgeState::Estimated),
        metadata.confidentiality,
        metadata.uu.into_values().collect(),
        metadata.parents.into_iter().collect(),
        metadata.anchors.into_iter().collect(),
    );
    Ok(ProductionReceipt {
        grinding: grinding.clone(),
        recovery: recovery.clone(),
        inputs: inputs.clone(),
        supply: supply.cloned(),
        points,
        claim,
        digest,
    })
}

fn put_distribution<D: Copy, B: Copy>(enc: &mut Encoder, value: &Distribution<Qty<D, B>>) {
    for number in [
        value.mean,
        value.minimum,
        value.p10,
        value.p50,
        value.p90,
        value.maximum,
    ] {
        put_number(enc, number.value());
    }
}

fn statistics<D: Copy, B: Copy>(
    points: &[ProductionPoint],
    value: impl Fn(&ProductionPoint) -> Qty<D, B>,
) -> Result<Distribution<Qty<D, B>>> {
    let mut sorted: Vec<_> = points.iter().collect();
    sorted.sort_by(|a, b| {
        value(a)
            .value()
            .total_cmp(&value(b).value())
            .then(a.scenario_id.cmp(&b.scenario_id))
    });
    let minimum = sorted
        .first()
        .ok_or_else(|| numerical("empty production population"))?;
    let maximum = sorted
        .last()
        .ok_or_else(|| numerical("empty production population"))?;
    let low = value(minimum).value();
    let high = value(maximum).value();
    let mut sum = 0.0;
    let mut correction = 0.0;
    for point in points {
        let term = product(numeric(point.weight)?, value(point).value() - low)?;
        let next = finite(sum + term)?;
        correction = finite(
            correction
                + if sum.abs() >= term.abs() {
                    (sum - next) + term
                } else {
                    (term - next) + sum
                },
        )?;
        sum = next;
    }
    let mean = finite(low + finite(sum + correction)?)?;
    if mean < low || mean > high {
        return Err(numerical("production mean escapes its population range"));
    }
    let quantile = |n, d| -> Result<Qty<D, B>> {
        let probability = ExactQty::new(n, d).map_err(arithmetic)?;
        let mut cumulative = ZERO;
        for point in &sorted {
            cumulative = cumulative.checked_add(point.weight).map_err(arithmetic)?;
            if !cumulative
                .checked_cmp(probability)
                .map_err(arithmetic)?
                .is_lt()
            {
                return Ok(value(point));
            }
        }
        Err(numerical("production quantile has no probability support"))
    };
    Ok(Distribution {
        mean: Qty::new(mean),
        minimum: value(minimum),
        p10: quantile(1, 10)?,
        p50: quantile(1, 2)?,
        p90: quantile(9, 10)?,
        maximum: value(maximum),
    })
}
