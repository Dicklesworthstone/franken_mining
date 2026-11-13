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
pub const NO_CLAIM: &str = "Conditional production over the complete declared shared scenarios. Grinding throughput already includes its declared utilization; scheduled hours are elapsed exposure and do not apply utilization again. Grade and product-size coordinates are explicitly linked to recovery inputs. The declared coupling basis connects the grinding blend to the recovery material domain and dry state. No transport lag, stock movement, feedback, physical blending, plant validation, fitted uncertainty, calibrated probabilities, forecast registration, payable metal, financial value or operating authorization is inferred.";
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessedDryTonnes;
pub type DryTonnes = Qty<ProcessedDryTonnes, Dry>;

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

/// Join all three populations by scenario identity and exact probability.
/// Grade matches the recovery coordinate exactly; grind size matches at the
/// admitted Bond binary64 precision, without a tolerance or unit conversion.
pub fn evaluate(
    grinding: &GrindingReceipt,
    recovery: &RecoveryEnsembleReceipt,
    inputs: &Claim<ProductionInputs>,
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
        let tonnes = product(gp.throughput.value(), numeric(row.scheduled_hours)?)?;
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
    let mut enc = Encoder::new_frame(*b"JPRD", 1);
    for source in [grinding.digest(), recovery.digest()] {
        enc.put_str(source);
    }
    metadata.include(grinding.claim())?;
    metadata.include(recovery.claim())?;
    metadata.include(inputs)?;
    inventory::put_source_envelope(&mut enc, grinding.claim(), 0);
    inventory::put_source_envelope(&mut enc, recovery.claim(), 1);
    inventory::put_source_envelope(&mut enc, inputs, 2);
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
        OUTPUT_METRIC,
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
