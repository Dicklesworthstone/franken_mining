//! DESIGN: declared N-tank residence-time transport in throughput-volume time
//! (plan §17.3, GB-41; bounded BD-313 slice). Source quantities remain extensive:
//! dry matter, H2O and total elements share one explicitly assumed passive RTD.
//! A known initial inventory is required; it never defaults to empty or steady.
//! No response-curve fit, noisy inverse, chemical reaction or plant calibration
//! is supplied. The optional recovery comparison declares delay then separation.
//! Determinism: DeterministicPerIsa

use std::collections::{BTreeMap, BTreeSet};

use crate::flowsheet::{ElementKey, FlowsheetScope, MaterialAmounts, validate_amounts};
use crate::inventory::{admit_source, put_source_envelope};
use crate::reconciliation::{Kilograms, Metadata, represented_rational};
use fmine_claim::{AdditivityClass, Claim, ClaimId};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, Qty, Volume};

pub use crate::flowsheet::reconciliation::MaterialChannel;
use kernel::{Sum, product, quotient, sum};

mod kernel;
pub mod recovery;
pub mod worksheet;

type Result<T> = std::result::Result<T, Refusal>;
pub const MODEL_METRIC: &str = "plant.rtd.model/1";
pub const OPENING_METRIC: &str = "plant.rtd.opening/1";
pub const FEED_METRIC: &str = "plant.rtd.feed/1";
pub const OUTPUT_METRIC: &str = "plant.rtd.transport/1";
pub const MAX_INTERVALS: usize = 4096;

/// Volumetric flow dimension, in m³/s, explicitly including the bypass branch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumetricFlow;
pub type CubicMetres = Qty<Volume>;
pub type CubicMetresPerSecond = Qty<VolumetricFlow>;
/// Every coordinate is kg; elemental channels overlap dry matter/water.
pub type ChannelMasses = BTreeMap<MaterialChannel, Kilograms>;

/// Source-qualified equal active tanks with a nonexchanging dead-volume stock.
#[derive(Debug, Clone, PartialEq)]
pub struct RtdModel {
    pub scope: FlowsheetScope,
    pub elements: BTreeSet<ElementKey>,
    pub tanks: usize,
    /// Geometric tank-train volume; active volume is (1-dead_fraction)*volume.
    pub volume_m3: CubicMetres,
    /// Fraction of TOTAL incoming slurry flow that skips the complete train.
    /// Actual tank flow is (1-bypass_fraction)*Q_total. At constant flow, the
    /// active-path mean residence is (1-dead_fraction)*V/[(1-bypass_fraction)*Q_total].
    /// In the plan's V_active/Q notation, Q means actual active-train flow.
    pub bypass_fraction: f64,
    pub dead_fraction: f64,
    /// Validity limits on each observed INTERVAL-MEAN total slurry flow.
    /// Integrated volume / elapsed time cannot establish pointwise bounds on
    /// unobserved within-interval Q(t); no such guarantee is returned.
    pub total_flow_domain_m3_per_s: [CubicMetresPerSecond; 2],
    /// Explicit common passive response for dry matter, H2O and every element.
    /// No phase segregation, reaction or distinct solid/liquid RTD is inferred.
    pub common_conservative_transport: bool,
    /// Feed composition is constant per unit slurry volume within each interval.
    pub piecewise_constant_feed_per_volume: bool,
    /// One declared absolute numerical/conservation allowance for every channel.
    pub closure_tolerances_kg: ChannelMasses,
    /// Shared finite work budget, including all intervals; at most 65536 steps.
    /// Analytic steps divide long throughputs into at most 32 tank turnovers.
    pub max_kernel_steps: usize,
}

impl RtdModel {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTDM", 1);
        put_scope(&mut enc, &self.scope);
        enc.put_u64(self.elements.len() as u64);
        for element in &self.elements {
            put_element(&mut enc, element);
        }
        enc.put_u64(self.tanks as u64);
        for value in [
            self.volume_m3.value(),
            self.bypass_fraction,
            self.dead_fraction,
            self.total_flow_domain_m3_per_s[0].value(),
            self.total_flow_domain_m3_per_s[1].value(),
        ] {
            enc.put_u64(value.to_bits());
        }
        enc.put_u8(u8::from(self.common_conservative_transport));
        enc.put_u8(u8::from(self.piecewise_constant_feed_per_volume));
        put_estimates(&mut enc, &self.closure_tolerances_kg);
        enc.put_u64(self.max_kernel_steps as u64);
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtdOpening {
    pub boundary: String,
    pub at_ns: i64,
    /// In flow order, first tank to last; all stocks explicitly supplied.
    pub tanks: Vec<MaterialAmounts>,
    /// No exchange with the active train in this declared dead-volume model.
    pub dead: MaterialAmounts,
}
impl RtdOpening {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTDO", 1);
        enc.put_str(&self.boundary);
        enc.put_i64(self.at_ns);
        enc.put_u64(self.tanks.len() as u64);
        for tank in &self.tanks {
            put_amounts(&mut enc, tank);
        }
        put_amounts(&mut enc, &self.dead);
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtdFeed {
    pub scope: FlowsheetScope,
    /// TOTAL integrated incoming slurry volume, including the bypass branch.
    /// Within-interval Q(t) may vary: only cumulative throughput enters the model.
    /// The source flow-domain check admits the interval mean, not every Q(t).
    pub slurry_volume_m3: CubicMetres,
    pub amounts: MaterialAmounts,
}
impl RtdFeed {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTDF", 1);
        put_scope(&mut enc, &self.scope);
        enc.put_u64(self.slurry_volume_m3.value().to_bits());
        put_amounts(&mut enc, &self.amounts);
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtdInterval {
    pub scope: FlowsheetScope,
    pub feed_source: ClaimId,
    /// Integral of actual active-train Q divided by one active tank volume.
    pub active_tank_turnovers: f64,
    pub opening_inventory_kg: ChannelMasses,
    pub feed_kg: ChannelMasses,
    pub bypass_kg: ChannelMasses,
    pub tank_outflow_kg: ChannelMasses,
    pub total_outflow_kg: ChannelMasses,
    pub closing_inventory_kg: ChannelMasses,
    /// Independently calculated opening+feed-outflow-closing, never redistributed.
    pub balance_residual_kg: ChannelMasses,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtdTransport {
    pub scope: FlowsheetScope,
    pub intervals: Vec<RtdInterval>,
    pub final_tanks_kg: Vec<ChannelMasses>,
    pub dead_inventory_kg: ChannelMasses,
    pub whole_window_residual_kg: ChannelMasses,
    pub kernel_steps: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtdReceipt {
    model: Claim<RtdModel>,
    opening: Claim<RtdOpening>,
    feeds: Vec<Claim<RtdFeed>>,
    claim: Claim<RtdTransport>,
    digest: String,
}
impl RtdReceipt {
    pub fn model(&self) -> &Claim<RtdModel> {
        &self.model
    }
    pub fn opening(&self) -> &Claim<RtdOpening> {
        &self.opening
    }
    pub fn feeds(&self) -> &[Claim<RtdFeed>] {
        &self.feeds
    }
    pub fn claim(&self) -> &Claim<RtdTransport> {
        &self.claim
    }
    pub fn summary(&self) -> &RtdTransport {
        &self.claim.value
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn fail(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant RTD: {}", detail.into()))
}
fn numerical(detail: impl Into<String>) -> Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, detail)
}
fn named(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
}
fn put_scope(enc: &mut Encoder, scope: &FlowsheetScope) {
    enc.put_str(&scope.boundary);
    enc.put_i64(scope.interval.start_ns);
    enc.put_i64(scope.interval.end_ns);
}
fn put_element(enc: &mut Encoder, element: &ElementKey) {
    enc.put_str(&element.element);
    enc.put_str(&element.measurand);
}
fn put_channel(enc: &mut Encoder, channel: &MaterialChannel) {
    match channel {
        MaterialChannel::DrySolids => enc.put_u8(0),
        MaterialChannel::Water => enc.put_u8(1),
        MaterialChannel::Element(element) => {
            enc.put_u8(2);
            put_element(enc, element);
        }
    }
}
fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}
fn put_amounts(enc: &mut Encoder, amounts: &MaterialAmounts) {
    put_qty(enc, amounts.dry_solids_kg);
    put_qty(enc, amounts.water_kg);
    enc.put_u64(amounts.elements_kg.len() as u64);
    for (key, value) in &amounts.elements_kg {
        put_element(enc, key);
        put_qty(enc, *value);
    }
}
fn put_estimates(enc: &mut Encoder, amounts: &ChannelMasses) {
    enc.put_u64(amounts.len() as u64);
    for (key, value) in amounts {
        put_channel(enc, key);
        enc.put_u64(value.value().to_bits());
    }
}
fn channel_order(model: &RtdModel) -> Vec<MaterialChannel> {
    [MaterialChannel::DrySolids, MaterialChannel::Water]
        .into_iter()
        .chain(model.elements.iter().cloned().map(MaterialChannel::Element))
        .collect()
}
fn estimates(keys: &[MaterialChannel], values: &[f64]) -> ChannelMasses {
    keys.iter()
        .cloned()
        .zip(values.iter().copied().map(Kilograms::new))
        .collect()
}

fn nominal(q: ExactQty, allowance: f64) -> Result<f64> {
    let value = q.to_f64();
    if !value.is_finite() || q.signum() < 0 || (q.signum() > 0 && value == 0.0) {
        return Err(numerical("source mass is unrepresentable"));
    }
    let loss = q
        .checked_sub(represented_rational(value)?)
        .map_err(|_| numerical("exact source-conversion comparison overflow"))?
        .to_f64()
        .abs();
    if loss > allowance {
        return Err(numerical(
            "source mass conversion exceeds declared precision budget",
        ));
    }
    Ok(value)
}

fn convert(amounts: &MaterialAmounts, model: &RtdModel, coordinates: usize) -> Result<Vec<f64>> {
    validate_amounts(amounts, &model.elements)?;
    let masses = [amounts.dry_solids_kg, amounts.water_kg]
        .into_iter()
        .chain(amounts.elements_kg.values().copied());
    masses
        .zip(model.closure_tolerances_kg.values())
        .map(|(q, tolerance)| nominal(q, tolerance.value() / (4 * coordinates) as f64))
        .collect()
}

fn admit_model(model: &RtdModel) -> Result<Vec<MaterialChannel>> {
    if !named(&model.scope.boundary) || model.scope.interval.start_ns >= model.scope.interval.end_ns
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "declare boundary and positive model interval",
        ));
    }
    let mut symbols = BTreeSet::new();
    for element in &model.elements {
        let s = element.element.as_bytes();
        if !(s.len() == 1 || s.len() == 2)
            || !s[0].is_ascii_uppercase()
            || (s.len() == 2 && !s[1].is_ascii_lowercase())
            || !named(&element.measurand)
            || !symbols.insert(element.element.clone())
        {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "distinct total elemental measurands required",
            ));
        }
    }
    let [low, high] = model.total_flow_domain_m3_per_s.map(|q| q.value());
    if !(1..=64).contains(&model.tanks)
        || model.elements.len() > 32
        || !(1..=65536).contains(&model.max_kernel_steps)
        || !model.volume_m3.value().is_finite()
        || model.volume_m3.value() <= 0.0
        || !model.bypass_fraction.is_finite()
        || !(0.0..=1.0).contains(&model.bypass_fraction)
        || !model.dead_fraction.is_finite()
        || !(0.0..1.0).contains(&model.dead_fraction)
        || !low.is_finite()
        || !high.is_finite()
        || low < 0.0
        || high <= 0.0
        || low > high
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "invalid tank, volume, fraction, flow-domain or work bounds",
        ));
    }
    if !model.common_conservative_transport || !model.piecewise_constant_feed_per_volume {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare common passive transport and constant composition per slurry volume",
        ));
    }
    let keys = channel_order(model);
    if !model.closure_tolerances_kg.keys().eq(keys.iter())
        || model
            .closure_tolerances_kg
            .values()
            .any(|q| !q.value().is_finite() || q.value() <= 0.0)
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "one positive kg allowance per declared channel is required",
        ));
    }
    Ok(keys)
}

fn inventory_total(stock: &[Vec<f64>], dead: &[f64]) -> Result<Vec<f64>> {
    dead.iter()
        .enumerate()
        .map(|(c, &amount)| sum(stock.iter().map(|row| row[c]).chain([amount])))
        .collect()
}

fn audit(
    opening: &[f64],
    feed: &[f64],
    output: &[f64],
    closing: &[f64],
    tolerances: &[f64],
) -> Result<Vec<f64>> {
    (0..feed.len())
        .map(|c| {
            let residual = sum([opening[c], feed[c], -output[c], -closing[c]])?;
            if residual.abs() > tolerances[c] {
                return Err(fail(
                    RefusalCode::Conservation101,
                    format!(
                        "channel {c} transport residual {residual} kg exceeds declared allowance"
                    ),
                ));
            }
            Ok(residual)
        })
        .collect()
}
fn poll(cancelled: &mut impl FnMut() -> bool) -> Result<()> {
    if cancelled() {
        Err(fail(
            RefusalCode::NoClaimInDomain201,
            "cancelled before complete RTD receipt",
        ))
    } else {
        Ok(())
    }
}

/// Propagate complete contiguous intervals in cumulative slurry-volume time.
///
/// Flow-domain admission concerns each interval's observed AVERAGE flow, not a
/// guarantee on unobserved within-interval excursions. All channels use the same
/// declared passive kernel. Exact outlet integration and retained stocks are
/// calculated separately. Failure/cancellation returns no partial receipt.
pub fn propagate(
    model: &Claim<RtdModel>,
    opening: &Claim<RtdOpening>,
    feeds: &[Claim<RtdFeed>],
    mut cancelled: impl FnMut() -> bool,
) -> Result<RtdReceipt> {
    poll(&mut cancelled)?;
    admit_source(
        model,
        MODEL_METRIC,
        &model.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    admit_source(
        opening,
        OPENING_METRIC,
        &opening.value.canonical_tag(),
        AdditivityClass::ExtensiveAdditive,
    )?;
    let m = &model.value;
    let keys = admit_model(m)?;
    if feeds.is_empty() || feeds.len() > MAX_INTERVALS {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "requires 1..4096 complete intervals",
        ));
    }
    if opening.value.boundary != m.scope.boundary
        || opening.value.at_ns != m.scope.interval.start_ns
        || opening.value.tanks.len() != m.tanks
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "opening must name every tank at the model's start and boundary",
        ));
    }
    let mut feeds = feeds.to_vec();
    feeds.sort_by_key(|f| {
        (
            f.value.scope.interval.start_ns,
            f.value.scope.interval.end_ns,
        )
    });
    let source_coordinates = feeds.len() + m.tanks + 1;
    let mut stock = opening
        .value
        .tanks
        .iter()
        .map(|a| convert(a, m, source_coordinates))
        .collect::<Result<Vec<_>>>()?;
    let dead = convert(&opening.value.dead, m, source_coordinates)?;
    if m.dead_fraction == 0.0 && dead.iter().any(|&q| q != 0.0) {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "nonzero dead stock requires nonzero dead volume",
        ));
    }
    let initial = inventory_total(&stock, &dead)?;
    let tank_volume = quotient(
        product(1.0 - m.dead_fraction, m.volume_m3.value())?,
        m.tanks as f64,
    )?;
    let tolerances: Vec<_> = m
        .closure_tolerances_kg
        .values()
        .map(|q| q.value())
        .collect();
    let mut metadata = Metadata::new();
    metadata.include(model)?;
    metadata.include(opening)?;
    let mut enc = Encoder::new_frame(*b"RTDR", 1);
    put_source_envelope(&mut enc, model, 0);
    put_source_envelope(&mut enc, opening, 1);
    enc.put_u64(feeds.len() as u64);
    let mut next_start = m.scope.interval.start_ns;
    let mut steps = 0;
    let mut results = Vec::with_capacity(feeds.len());
    let mut all_feed: Vec<_> = keys.iter().map(|_| Sum::default()).collect();
    let mut all_output: Vec<_> = keys.iter().map(|_| Sum::default()).collect();
    for source in &feeds {
        poll(&mut cancelled)?;
        admit_source(
            source,
            FEED_METRIC,
            &source.value.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?;
        let f = &source.value;
        let t = f.scope.interval;
        if f.scope.boundary != m.scope.boundary
            || t.start_ns != next_start
            || t.end_ns <= t.start_ns
            || t.end_ns > m.scope.interval.end_ns
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "feed intervals must exactly tile the declared boundary/window",
            ));
        }
        let elapsed_ns = t
            .end_ns
            .checked_sub(t.start_ns)
            .ok_or_else(|| numerical("interval duration overflows"))?;
        let elapsed_s = elapsed_ns as f64 * 1e-9;
        let volume = f.slurry_volume_m3.value();
        if !volume.is_finite() || volume < 0.0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "slurry volume must be finite and nonnegative",
            ));
        }
        let flow = quotient(volume, elapsed_s)?;
        if flow < m.total_flow_domain_m3_per_s[0].value()
            || flow > m.total_flow_domain_m3_per_s[1].value()
        {
            return Err(fail(
                RefusalCode::OutsideAoa203,
                "interval average total slurry flow is outside source domain",
            ));
        }
        let input = convert(&f.amounts, m, source_coordinates)?;
        if volume == 0.0 && input.iter().any(|&q| q != 0.0) {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "nonzero feed mass requires nonzero slurry throughput",
            ));
        }
        let before = inventory_total(&stock, &dead)?;
        let x = quotient(product(1.0 - m.bypass_fraction, volume)?, tank_volume)?;
        let bypass = input
            .iter()
            .map(|&q| product(m.bypass_fraction, q))
            .collect::<Result<Vec<_>>>()?;
        let mut discharged: Vec<_> = keys.iter().map(|_| Sum::default()).collect();
        if x > 0.0 {
            let count = (x / kernel::MAX_TURNOVER).ceil().max(1.0);
            if !count.is_finite() || count > (m.max_kernel_steps - steps) as f64 {
                return Err(numerical(
                    "complete RTD trajectory exceeds declared work budget",
                ));
            }
            let count = count as usize;
            let weights = kernel::weights(m.tanks, quotient(x, count as f64)?)?;
            let step_feed = input
                .iter()
                .map(|&q| quotient(product(1.0 - m.bypass_fraction, q)?, count as f64))
                .collect::<Result<Vec<_>>>()?;
            for _ in 0..count {
                poll(&mut cancelled)?;
                let out = kernel::advance(&mut stock, &step_feed, &weights)?;
                for (sum, value) in discharged.iter_mut().zip(out) {
                    sum.add(value)?;
                }
            }
            steps += count;
        }
        let out = discharged
            .iter()
            .map(Sum::finish)
            .collect::<Result<Vec<_>>>()?;
        let total = out
            .iter()
            .zip(&bypass)
            .map(|(&a, &b)| sum([a, b]))
            .collect::<Result<Vec<_>>>()?;
        let after = inventory_total(&stock, &dead)?;
        let residual = audit(&before, &input, &total, &after, &tolerances)?;
        for c in 0..keys.len() {
            all_feed[c].add(input[c])?;
            all_output[c].add(total[c])?;
        }
        let interval = RtdInterval {
            scope: f.scope.clone(),
            feed_source: source.claim_id,
            active_tank_turnovers: x,
            opening_inventory_kg: estimates(&keys, &before),
            feed_kg: estimates(&keys, &input),
            bypass_kg: estimates(&keys, &bypass),
            tank_outflow_kg: estimates(&keys, &out),
            total_outflow_kg: estimates(&keys, &total),
            closing_inventory_kg: estimates(&keys, &after),
            balance_residual_kg: estimates(&keys, &residual),
        };
        metadata.include(source)?;
        put_source_envelope(&mut enc, source, 2);
        enc.put_u64(x.to_bits());
        for amounts in [
            &interval.opening_inventory_kg,
            &interval.feed_kg,
            &interval.bypass_kg,
            &interval.tank_outflow_kg,
            &interval.total_outflow_kg,
            &interval.closing_inventory_kg,
            &interval.balance_residual_kg,
        ] {
            put_estimates(&mut enc, amounts);
        }
        results.push(interval);
        next_start = t.end_ns;
    }
    if next_start != m.scope.interval.end_ns {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "missing final feed interval",
        ));
    }
    poll(&mut cancelled)?;
    let whole_residual = audit(
        &initial,
        &all_feed
            .iter()
            .map(Sum::finish)
            .collect::<Result<Vec<_>>>()?,
        &all_output
            .iter()
            .map(Sum::finish)
            .collect::<Result<Vec<_>>>()?,
        &inventory_total(&stock, &dead)?,
        &tolerances,
    )?;
    let final_tanks: Vec<_> = stock.iter().map(|row| estimates(&keys, row)).collect();
    for tank in &final_tanks {
        put_estimates(&mut enc, tank);
    }
    let whole_window_residual_kg = estimates(&keys, &whole_residual);
    put_estimates(&mut enc, &whole_window_residual_kg);
    enc.put_u64(steps as u64);
    let digest = blake3_hash(&enc.finish()).to_hex();
    let value = RtdTransport {
        scope: m.scope.clone(),
        intervals: results,
        final_tanks_kg: final_tanks,
        dead_inventory_kg: estimates(&keys, &dead),
        whole_window_residual_kg,
        kernel_steps: steps,
    };
    Ok(RtdReceipt {
        model: model.clone(),
        opening: opening.clone(),
        feeds,
        claim: metadata.into_claim(OUTPUT_METRIC, value, &digest),
        digest,
    })
}

#[cfg(test)]
mod tests;
