//! DESIGN: conditional Bond grinding energy and power-limited throughput at
//! declared processing-blend support (plan §17.3, PO-CRUC-4; BD-176/181).
//! Complete simultaneous scenarios preserve input dependence. Their weights
//! describe alternative states of one blend, never physical blending weights.
//! This evaluates assumptions; it neither fits nor calibrates a plant model.
//! Determinism: DeterministicPerIsa

use crate::{inventory, reconciliation::Metadata};
use fmine_claim::{AdditivityClass, Claim, ClaimId, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_time::ValidTime;
use fmine_units::{Dry, ExactQty, Qty, UnitsError};

mod render;
#[cfg(test)]
mod tests;
pub mod worksheet;

type Result<T> = std::result::Result<T, Refusal>;
pub const MODEL_FAMILY: &str = "bond-metric-dry/1";
pub const MODEL_METRIC: &str = "plant.grinding.bond_model/1";
pub const INPUT_METRIC: &str = "plant.grinding.joint_scenario/1";
pub const OUTPUT_METRIC: &str = "plant.grinding.energy_and_throughput/1";
pub const MAX_SCENARIOS: usize = 4096;

/// Bond's metric work index and computed specific energy use kWh per dry metric tonne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecificEnergy;
pub type KwhPerDryTonne = Qty<SpecificEnergy, Dry>;
/// Particle-size coordinates are micrometres, not metres or millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleSize;
pub type Micrometres = Qty<ParticleSize>;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Power;
pub type Kilowatts = Qty<Power>;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DryThroughput;
pub type DryTonnesPerHour = Qty<DryThroughput, Dry>;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Utilization;
pub type Fraction = Qty<Utilization>;

/// One circuit and processing blend over one positive physical interval.
/// No sample-to-blend or block-to-blend transfer is inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrindingScope {
    pub boundary: String,
    pub circuit: String,
    pub blend: String,
    pub interval: ValidTime,
}

/// A versioned declared validity box. The work-index test basis must describe
/// the closing screen/feed preparation or name the source that establishes it.
/// Domains are closed; their declaration does not demonstrate empirical validity.
#[derive(Debug, Clone, PartialEq)]
pub struct BondModel {
    pub scope: GrindingScope,
    pub id: String,
    pub version: String,
    pub work_index_test_basis: String,
    pub validity_basis: String,
    pub complete_joint_population: bool,
    pub work_index_domain: [KwhPerDryTonne; 2],
    pub feed_size_domain: [Micrometres; 2],
    pub product_size_domain: [Micrometres; 2],
    pub power_domain: [Kilowatts; 2],
    pub utilization_domain: [Fraction; 2],
}

impl BondModel {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"BMOD", 1);
        enc.put_str(MODEL_FAMILY);
        put_scope(&mut enc, &self.scope);
        for text in [
            &self.id,
            &self.version,
            &self.work_index_test_basis,
            &self.validity_basis,
        ] {
            enc.put_str(text);
        }
        enc.put_u8(u8::from(self.complete_joint_population));
        for bounds in self.domains() {
            for value in bounds {
                put_number(&mut enc, value);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }

    fn domains(&self) -> [[f64; 2]; 5] {
        [
            self.work_index_domain.map(Qty::value),
            self.feed_size_domain.map(Qty::value),
            self.product_size_domain.map(Qty::value),
            self.power_domain.map(Qty::value),
            self.utilization_domain.map(Qty::value),
        ]
    }
}

/// All inputs in a row occur together. Exact positive probabilities must sum
/// to one; no independence, omitted state, normalization or blending is inferred.
#[derive(Debug, Clone, PartialEq)]
pub struct GrindingScenario {
    pub id: String,
    pub scope: GrindingScope,
    pub weight: ExactQty,
    pub work_index: KwhPerDryTonne,
    pub feed_size: Micrometres,
    pub product_size: Micrometres,
    pub installed_power: Kilowatts,
    pub utilization: Fraction,
}

impl GrindingScenario {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"BSCN", 1);
        enc.put_str(&self.id);
        put_scope(&mut enc, &self.scope);
        put_exact(&mut enc, self.weight);
        for value in self.coordinates() {
            put_number(&mut enc, value);
        }
        blake3_hash(&enc.finish()).to_hex()
    }

    fn coordinates(&self) -> [f64; 5] {
        [
            self.work_index.value(),
            self.feed_size.value(),
            self.product_size.value(),
            self.installed_power.value(),
            self.utilization.value(),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingState {
    Operating,
    NoInstalledPower,
    ZeroUtilization,
    NoPowerAndZeroUtilization,
}
impl OperatingState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Operating => "operating",
            Self::NoInstalledPower => "shutdown_no_installed_power",
            Self::ZeroUtilization => "shutdown_zero_utilization",
            Self::NoPowerAndZeroUtilization => "shutdown_no_power_and_zero_utilization",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrindingPoint {
    pub scenario_id: String,
    pub source: ClaimId,
    pub weight: ExactQty,
    pub specific_energy: KwhPerDryTonne,
    pub effective_power: Kilowatts,
    pub throughput: DryTonnesPerHour,
    pub state: OperatingState,
}

/// Conditional finite-population statistics, not calibrated confidence bounds.
/// Quantiles are the inverse CDF under exact weights, without interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct Distribution<T> {
    pub mean: T,
    pub minimum: T,
    pub p10: T,
    pub p50: T,
    pub p90: T,
    pub maximum: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrindingSummary {
    pub scope: GrindingScope,
    pub scenario_count: usize,
    pub specific_energy: Distribution<KwhPerDryTonne>,
    pub throughput: Distribution<DryTonnesPerHour>,
    pub shutdown_probability: ExactQty,
}

/// Complete immutable sources and results. A bad member returns no partial
/// receipt, even if earlier members were numerically evaluable.
#[derive(Debug, Clone, PartialEq)]
pub struct GrindingReceipt {
    model: Claim<BondModel>,
    scenarios: Vec<Claim<GrindingScenario>>,
    points: Vec<GrindingPoint>,
    claim: Claim<GrindingSummary>,
    digest: String,
}
impl GrindingReceipt {
    pub fn model(&self) -> &Claim<BondModel> {
        &self.model
    }
    pub fn scenarios(&self) -> &[Claim<GrindingScenario>] {
        &self.scenarios
    }
    pub fn points(&self) -> &[GrindingPoint] {
        &self.points
    }
    pub fn claim(&self) -> &Claim<GrindingSummary> {
        &self.claim
    }
    pub fn summary(&self) -> &GrindingSummary {
        &self.claim.value
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

const AXES: [&str; 5] = [
    "work_index_kwh_per_dry_t",
    "f80_microns",
    "p80_microns",
    "installed_power_kw",
    "utilization",
];

fn fail(code: RefusalCode, message: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("Bond grinding: {}", message.into()))
}
fn numerical(message: impl Into<String>) -> Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, message)
}
fn arithmetic(_: UnitsError) -> Refusal {
    numerical("exact probability arithmetic overflows")
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}
fn put_number(enc: &mut Encoder, value: f64) {
    enc.put_u64(if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    });
}
fn put_exact(enc: &mut Encoder, value: ExactQty) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}
fn put_scope(enc: &mut Encoder, scope: &GrindingScope) {
    for text in [&scope.boundary, &scope.circuit, &scope.blend] {
        enc.put_str(text);
    }
    enc.put_i64(scope.interval.start_ns);
    enc.put_i64(scope.interval.end_ns);
}
fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(if value == 0.0 { 0.0 } else { value })
    } else {
        Err(numerical("nonfinite numerical result"))
    }
}
fn product(a: f64, b: f64) -> Result<f64> {
    let value = finite(a * b)?;
    if a != 0.0 && b != 0.0 && value == 0.0 {
        return Err(numerical("positive product underflows"));
    }
    Ok(value)
}
fn scope_valid(scope: &GrindingScope) -> Result<()> {
    if [&scope.boundary, &scope.circuit, &scope.blend]
        .iter()
        .any(|s| !named(s))
        || scope.interval.start_ns >= scope.interval.end_ns
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "name the boundary, circuit and processing blend over a positive interval",
        ));
    }
    Ok(())
}
fn validate_model(model: &Claim<BondModel>) -> Result<()> {
    inventory::admit_source(
        model,
        MODEL_METRIC,
        &model.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let m = &model.value;
    scope_valid(&m.scope)?;
    if [
        &m.id,
        &m.version,
        &m.work_index_test_basis,
        &m.validity_basis,
    ]
    .iter()
    .any(|s| !named(s))
        || m.id.len() > 256
        || m.version.len() > 256
        || !m.complete_joint_population
    {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare the versioned card, work-index test basis, validity basis and complete joint population",
        ));
    }
    for (axis, [lo, hi]) in m.domains().iter().copied().enumerate() {
        if !lo.is_finite()
            || !hi.is_finite()
            || lo > hi
            || (axis < 3 && lo <= 0.0)
            || (axis >= 3 && lo < 0.0)
            || (axis == 4 && hi > 1.0)
        {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                format!("{} has an invalid closed validity domain", AXES[axis]),
            ));
        }
    }
    Ok(())
}
fn evaluate_point(model: &BondModel, source: &Claim<GrindingScenario>) -> Result<GrindingPoint> {
    inventory::admit_source(
        source,
        INPUT_METRIC,
        &source.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let s = &source.value;
    if s.scope != model.scope {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "scenario and model must have the same boundary, circuit, blend and interval",
        ));
    }
    let [wi, feed, size, power, utilization] = s.coordinates();
    if s.coordinates().iter().any(|v| !v.is_finite())
        || wi <= 0.0
        || size <= 0.0
        || feed <= size
        || power < 0.0
        || !(0.0..=1.0).contains(&utilization)
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "require finite Wi > 0, F80 > P80 > 0, installed power >= 0 and utilization in [0,1]",
        ));
    }
    for (axis, (value, [lo, hi])) in s.coordinates().into_iter().zip(model.domains()).enumerate() {
        if value < lo || value > hi {
            return Err(fail(
                RefusalCode::OutsideAoa203,
                format!(
                    "{}={value} lies outside declared [{lo}, {hi}]; no extrapolation",
                    AXES[axis]
                ),
            ));
        }
    }
    // Algebraically 1/sqrt(P)-1/sqrt(F), with no subtracting close square
    // roots/reciprocals. The supplied F-P is positive for distinct finite inputs.
    let root_p = size.sqrt();
    let reduction = ((feed - size) / feed) / (1.0 + root_p / feed.sqrt());
    let energy = product(product(wi, 10.0 / root_p)?, reduction)?;
    if energy <= 0.0 {
        return Err(numerical("specific energy is not representably positive"));
    }
    let effective_power = product(power, utilization)?;
    let throughput = finite(effective_power / energy)?;
    if effective_power > 0.0 && throughput == 0.0 {
        return Err(numerical("positive throughput underflows"));
    }
    let state = match (power == 0.0, utilization == 0.0) {
        (false, false) => OperatingState::Operating,
        (true, false) => OperatingState::NoInstalledPower,
        (false, true) => OperatingState::ZeroUtilization,
        (true, true) => OperatingState::NoPowerAndZeroUtilization,
    };
    Ok(GrindingPoint {
        scenario_id: s.id.clone(),
        source: source.claim_id,
        weight: s.weight,
        specific_energy: KwhPerDryTonne::new(energy),
        effective_power: Kilowatts::new(effective_power),
        throughput: DryTonnesPerHour::new(throughput),
        state,
    })
}

/// Evaluate all admitted scenarios before taking any moments. A throughput
/// mean is sum(weight * throughput), never throughput at averaged hardness.
pub fn evaluate(
    model: &Claim<BondModel>,
    scenarios: &[Claim<GrindingScenario>],
) -> Result<GrindingReceipt> {
    validate_model(model)?;
    if scenarios.is_empty() || scenarios.len() > MAX_SCENARIOS {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "requires 1..4096 complete joint scenarios",
        ));
    }
    let mut scenarios = scenarios.to_vec();
    scenarios.sort_by(|a, b| a.value.id.cmp(&b.value.id));
    let mut total = ExactQty::from_integer(0);
    for (index, s) in scenarios.iter().enumerate() {
        if !named(&s.value.id)
            || s.value.id.len() > 256
            || (index > 0 && scenarios[index - 1].value.id == s.value.id)
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "scenario IDs must be unique nonblank names of at most 256 bytes",
            ));
        }
        if s.value.weight.signum() <= 0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "every scenario needs strictly positive exact probability",
            ));
        }
        total = total.checked_add(s.value.weight).map_err(arithmetic)?;
    }
    if total != ExactQty::from_integer(1) {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "probabilities sum to {}/{}; exactly one is required without renormalization",
                total.num(),
                total.den()
            ),
        ));
    }
    let mut enc = Encoder::new_frame(*b"BOND", 1);
    enc.put_str(MODEL_FAMILY);
    let mut metadata = Metadata::new();
    metadata.evidence = EvidenceClass::ModelledUncalibrated;
    metadata.include(model)?;
    inventory::put_source_envelope(&mut enc, model, 0);
    enc.put_u64(scenarios.len() as u64);
    let mut points = Vec::with_capacity(scenarios.len());
    let mut shutdown_probability = ExactQty::from_integer(0);
    for source in &scenarios {
        let point = evaluate_point(&model.value, source).map_err(|mut refusal| {
            refusal.what = format!("scenario {}: {}", source.value.id, refusal.what);
            refusal
        })?;
        metadata.include(source)?;
        inventory::put_source_envelope(&mut enc, source, 1);
        for value in [
            point.specific_energy.value(),
            point.effective_power.value(),
            point.throughput.value(),
        ] {
            put_number(&mut enc, value);
        }
        enc.put_str(point.state.as_str());
        if point.state != OperatingState::Operating {
            shutdown_probability = shutdown_probability
                .checked_add(point.weight)
                .map_err(arithmetic)?;
        }
        points.push(point);
    }
    let specific_energy = statistics(&points, |p| p.specific_energy)?;
    let throughput = statistics(&points, |p| p.throughput)?;
    put_distribution(&mut enc, &specific_energy);
    put_distribution(&mut enc, &throughput);
    put_exact(&mut enc, shutdown_probability);
    let digest = blake3_hash(&enc.finish()).to_hex();
    // Representation's scalar statistics describe throughput. Specific-energy
    // statistics remain separately typed in the payload and both renderings.
    let representation = Representation::EnsembleSummary {
        n: points.len() as u32,
        mean: throughput.mean.value(),
        p10: throughput.p10.value(),
        p50: throughput.p50.value(),
        p90: throughput.p90.value(),
    };
    let summary = GrindingSummary {
        scope: model.value.scope.clone(),
        scenario_count: points.len(),
        specific_energy,
        throughput,
        shutdown_probability,
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
    Ok(GrindingReceipt {
        model: model.clone(),
        scenarios,
        points,
        claim,
        digest,
    })
}

fn put_distribution<D: Copy>(enc: &mut Encoder, d: &Distribution<Qty<D, Dry>>) {
    for q in [d.mean, d.minimum, d.p10, d.p50, d.p90, d.maximum] {
        put_number(enc, q.value());
    }
}

fn statistics<D: Copy>(
    points: &[GrindingPoint],
    value: impl Fn(&GrindingPoint) -> Qty<D, Dry>,
) -> Result<Distribution<Qty<D, Dry>>> {
    let mut sorted: Vec<_> = points.iter().collect();
    sorted.sort_by(|a, b| {
        value(a)
            .value()
            .total_cmp(&value(b).value())
            .then_with(|| a.scenario_id.cmp(&b.scenario_id))
    });
    let first = sorted
        .first()
        .ok_or_else(|| numerical("empty weighted population"))?;
    let last = sorted
        .last()
        .ok_or_else(|| numerical("empty weighted population"))?;
    let minimum = value(first).value();
    let maximum = value(last).value();
    let mut sum = Sum::default();
    for p in points {
        sum.add(product(p.weight.to_f64(), value(p).value() - minimum)?)?;
    }
    let mean = finite(minimum + sum.finish()?)?;
    if mean < minimum || mean > maximum {
        return Err(numerical("weighted mean escapes its population range"));
    }
    let quantile = |n, d| -> Result<Qty<D, Dry>> {
        let probability = ExactQty::new(n, d).map_err(arithmetic)?;
        let mut cumulative = ExactQty::from_integer(0);
        for p in &sorted {
            cumulative = cumulative.checked_add(p.weight).map_err(arithmetic)?;
            if !cumulative
                .checked_cmp(probability)
                .map_err(arithmetic)?
                .is_lt()
            {
                return Ok(value(p));
            }
        }
        Err(numerical("quantile has no probability support"))
    };
    Ok(Distribution {
        mean: Qty::new(mean),
        minimum: Qty::new(minimum),
        p10: quantile(1, 10)?,
        p50: quantile(1, 2)?,
        p90: quantile(9, 10)?,
        maximum: Qty::new(maximum),
    })
}

/// Fixed-order compensated sum; intermediate overflow is a refusal.
#[derive(Default)]
struct Sum {
    sum: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) -> Result<()> {
        let next = finite(self.sum + value)?;
        self.correction = finite(
            self.correction
                + if self.sum.abs() >= value.abs() {
                    (self.sum - next) + value
                } else {
                    (value - next) + self.sum
                },
        )?;
        self.sum = next;
        Ok(())
    }
    fn finish(self) -> Result<f64> {
        finite(self.sum + self.correction)
    }
}
