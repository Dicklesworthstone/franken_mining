//! DESIGN: bounded, known-kernel inverse residence-time transport (BD-313).
//! Genuine feed/outlet observations retain their full declared covariance;
//! explicitly unmeasured feeds receive no invented observation or prior.
//! Initial inventories, integrated volumes and the passive kernel are fixed
//! conditional inputs. No parameter fitting or inverse regularization occurs.
//! Determinism: DeterministicPerIsa

use super::*;
use crate::flowsheet::reconciliation::unmeasured::GrossErrorDiagnostic;
use crate::reconciliation::{KilogramsSquared, nominal_with_precision_budget};
use fmine_numerics::reconciliation::{
    partial::{PartialReconciliation, reconcile_with_unmeasured},
    reconcile_linear,
};

pub mod worksheet;

pub const MAX_INTERVALS: usize = 32;
pub const INPUT_METRIC: &str = "plant.rtd.inverse_interval/1";
pub const ERROR_MODEL_METRIC: &str = "plant.rtd.inverse_error_model/1";
pub const OUTPUT_METRIC: &str = "plant.rtd.inverse_reconciliation/1";
/// Bounds response construction as well as the scalar replay, before work begins.
pub const MAX_RESPONSE_WORK: usize = 4_194_304;

/// Complete integrated elemental outlet plus either a genuine feed observation
/// or an explicit unmeasured feed. Both masses include the bypass branch.
#[derive(Debug, Clone, PartialEq)]
pub struct InverseInterval {
    pub scope: FlowsheetScope,
    pub element: ElementKey,
    pub slurry_volume_m3: CubicMetres,
    pub feed_kg: Option<ExactQty>,
    pub outlet_kg: ExactQty,
}
impl InverseInterval {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTII", 1);
        put_scope(&mut enc, &self.scope);
        put_element(&mut enc, &self.element);
        enc.put_u64(self.slurry_volume_m3.value().to_bits());
        enc.put_u8(u8::from(self.feed_kg.is_some()));
        if let Some(value) = self.feed_kg {
            put_qty(&mut enc, value);
        }
        put_qty(&mut enc, self.outlet_kg);
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationRole {
    Feed,
    Outlet,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InverseObservation {
    pub interval_source: ClaimId,
    pub role: ObservationRole,
}

/// Conditional measurement-error model in canonical genuine-observation order.
/// Missing feeds have no covariance row. Every cross-period pair is explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct InverseErrorModel {
    pub scope: FlowsheetScope,
    pub element: ElementKey,
    pub model_source: ClaimId,
    pub opening_source: ClaimId,
    pub observations: Vec<InverseObservation>,
    pub covariance: Vec<KilogramsSquared>,
    pub error_model_basis: String,
    pub fixed_transport_and_opening: bool,
    pub unbiased_gaussian_errors: bool,
    pub global_chi_square_limit: f64,
    pub normalized_adjustment_limit: Option<f64>,
    pub closure_tolerance_kg: Kilograms,
}
impl InverseErrorModel {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"RTIE", 1);
        put_scope(&mut enc, &self.scope);
        put_element(&mut enc, &self.element);
        enc.put_str(&self.model_source.to_string());
        enc.put_str(&self.opening_source.to_string());
        enc.put_u64(self.observations.len() as u64);
        for key in &self.observations {
            enc.put_str(&key.interval_source.to_string());
            enc.put_u8(match key.role {
                ObservationRole::Feed => 0,
                ObservationRole::Outlet => 1,
            });
        }
        enc.put_u64(self.covariance.len() as u64);
        for value in &self.covariance {
            enc.put_u64(value.value().to_bits());
        }
        enc.put_str(&self.error_model_basis);
        enc.put_u8(u8::from(self.fixed_transport_and_opening));
        enc.put_u8(u8::from(self.unbiased_gaussian_errors));
        enc.put_u64(self.global_chi_square_limit.to_bits());
        put_optional(&mut enc, self.normalized_adjustment_limit);
        enc.put_u64(self.closure_tolerance_kg.value().to_bits());
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// The complete conditional solution, including diagnostics for rejected fits.
#[derive(Debug, Clone, PartialEq)]
pub struct InverseReconciliation {
    pub scope: FlowsheetScope,
    pub element: ElementKey,
    pub observations: Vec<InverseObservation>,
    /// Chronological source identities of the explicitly missing feeds.
    pub unmeasured_feeds: Vec<ClaimId>,
    pub observed_kg: Vec<Kilograms>,
    /// Genuine measured coordinates first, then missing feeds chronologically.
    pub estimates_kg: Vec<Kilograms>,
    pub measured_adjustments_kg: Vec<Kilograms>,
    /// Full row-major covariance in `estimates_kg` order; correlated, not diagonal.
    pub posterior_covariance: Vec<KilogramsSquared>,
    pub measured_adjustment_covariance: Vec<KilogramsSquared>,
    pub normalized_adjustments: Vec<Option<f64>>,
    pub feed_coordinate_indices: Vec<usize>,
    pub outlet_coordinate_indices: Vec<usize>,
    /// Causal N-by-N outlet response to integrated interval feed masses.
    pub response_matrix: Vec<f64>,
    /// Fixed initial-stock contribution, independently propagated through the train.
    pub opening_outlet_kg: Vec<Kilograms>,
    pub fitted_feed_kg: Vec<Kilograms>,
    pub fitted_outlet_kg: Vec<Kilograms>,
    /// Scalar forward replay of fitted feeds, separate from the inverse solve.
    pub predicted_outlet_kg: Vec<Kilograms>,
    /// Total active plus fixed dead elemental inventory after each interval.
    pub closing_inventory_kg: Vec<Kilograms>,
    /// Only the measured contribution when a feed is missing, not a raw imbalance.
    pub raw_observed_residual_kg: Vec<Kilograms>,
    pub transport_residual_kg: Vec<Kilograms>,
    /// Independently propagated opening+feed-outlet-closing for each interval.
    pub conservation_residual_kg: Vec<Kilograms>,
    pub whole_window_residual_kg: Kilograms,
    pub gross_error: GrossErrorDiagnostic,
    pub kernel_steps: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InverseReceipt {
    model: Claim<RtdModel>,
    opening: Claim<RtdOpening>,
    intervals: Vec<Claim<InverseInterval>>,
    error_model: Claim<InverseErrorModel>,
    claim: Claim<InverseReconciliation>,
    digest: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RejectedInverse {
    model: Claim<RtdModel>,
    opening: Claim<RtdOpening>,
    intervals: Vec<Claim<InverseInterval>>,
    error_model: Claim<InverseErrorModel>,
    diagnostics: InverseReconciliation,
    refusal: Refusal,
    digest: String,
}
#[derive(Debug, Clone, PartialEq)]
pub enum InverseOutcome {
    Admitted(Box<InverseReceipt>),
    Rejected(Box<RejectedInverse>),
}
impl InverseOutcome {
    pub fn model(&self) -> &Claim<RtdModel> {
        match self {
            Self::Admitted(r) => &r.model,
            Self::Rejected(r) => &r.model,
        }
    }
    pub fn opening(&self) -> &Claim<RtdOpening> {
        match self {
            Self::Admitted(r) => &r.opening,
            Self::Rejected(r) => &r.opening,
        }
    }
    pub fn intervals(&self) -> &[Claim<InverseInterval>] {
        match self {
            Self::Admitted(r) => &r.intervals,
            Self::Rejected(r) => &r.intervals,
        }
    }
    pub fn error_model(&self) -> &Claim<InverseErrorModel> {
        match self {
            Self::Admitted(r) => &r.error_model,
            Self::Rejected(r) => &r.error_model,
        }
    }
    pub fn diagnostics(&self) -> &InverseReconciliation {
        match self {
            Self::Admitted(r) => &r.claim.value,
            Self::Rejected(r) => &r.diagnostics,
        }
    }
    pub fn digest(&self) -> &str {
        match self {
            Self::Admitted(r) => &r.digest,
            Self::Rejected(r) => &r.digest,
        }
    }
    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            Self::Admitted(_) => None,
            Self::Rejected(r) => Some(&r.refusal),
        }
    }
    pub fn admitted_claim(&self) -> Result<&Claim<InverseReconciliation>> {
        match self {
            Self::Admitted(r) => Ok(&r.claim),
            Self::Rejected(r) => Err(r.refusal.clone()),
        }
    }
}

fn put_optional(enc: &mut Encoder, value: Option<f64>) {
    enc.put_u8(u8::from(value.is_some()));
    if let Some(value) = value {
        enc.put_u64(value.to_bits());
    }
}
fn ordered(intervals: &[Claim<InverseInterval>]) -> Result<Vec<&Claim<InverseInterval>>> {
    if intervals.is_empty() || intervals.len() > MAX_INTERVALS {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "inverse requires 1..32 complete intervals",
        ));
    }
    let mut rows: Vec<_> = intervals.iter().collect();
    rows.sort_by_key(|row| {
        (
            row.value.scope.interval.start_ns,
            row.value.scope.interval.end_ns,
        )
    });
    for row in &rows {
        admit_source(
            row,
            INPUT_METRIC,
            &row.value.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?;
        if row.value.scope.interval.start_ns >= row.value.scope.interval.end_ns
            || row.value.outlet_kg.signum() < 0
            || row.value.feed_kg.is_some_and(|q| q.signum() < 0)
        {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "inverse observations require positive intervals and nonnegative integrated masses",
            ));
        }
    }
    for pair in rows.windows(2) {
        if pair[0].value.scope.boundary != pair[1].value.scope.boundary
            || pair[0].value.element != pair[1].value.element
            || pair[0].value.scope.interval.end_ns != pair[1].value.scope.interval.start_ns
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "inverse intervals must form one contiguous elemental boundary history",
            ));
        }
    }
    Ok(rows)
}

/// Canonical chronological order: observed feed when present, then outlet.
pub fn observation_order(intervals: &[Claim<InverseInterval>]) -> Result<Vec<InverseObservation>> {
    let mut result = Vec::new();
    for row in ordered(intervals)? {
        if row.value.feed_kg.is_some() {
            result.push(InverseObservation {
                interval_source: row.claim_id,
                role: ObservationRole::Feed,
            });
        }
        result.push(InverseObservation {
            interval_source: row.claim_id,
            role: ObservationRole::Outlet,
        });
    }
    Ok(result)
}

struct Transition {
    count: usize,
    weights: Option<kernel::Weights>,
}
struct Response {
    matrix: Vec<f64>,
    initial_outlet: Vec<f64>,
    initial_stocks: Vec<f64>,
    dead: f64,
    transitions: Vec<Transition>,
    steps: usize,
}

fn response(
    model: &RtdModel,
    opening: &RtdOpening,
    intervals: &[&Claim<InverseInterval>],
    element: &ElementKey,
    precision: f64,
    cancelled: &mut impl FnMut() -> bool,
) -> Result<Response> {
    let count = intervals.len();
    let keys = admit_model(model)?;
    let channel = keys
        .iter()
        .position(|key| key == &MaterialChannel::Element(element.clone()))
        .ok_or_else(|| {
            fail(
                RefusalCode::BasisMismatch105,
                "inverse element is absent from transport model",
            )
        })?;
    if opening.boundary != model.scope.boundary
        || opening.at_ns != model.scope.interval.start_ns
        || opening.tanks.len() != model.tanks
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "inverse opening must explicitly name every initial tank at the model boundary/start",
        ));
    }
    let selected = |amounts: &MaterialAmounts| -> Result<f64> {
        let values = convert(amounts, model, count + model.tanks + 1)?;
        let exact = amounts
            .elements_kg
            .get(element)
            .ok_or_else(|| fail(RefusalCode::BasisMismatch105, "missing opening element"))?;
        let value = nominal(*exact, precision / (model.tanks + 1) as f64)?;
        if value != values[channel] {
            return Err(numerical("inconsistent initial mass conversion"));
        }
        Ok(value)
    };
    let initial_stocks = opening
        .tanks
        .iter()
        .map(selected)
        .collect::<Result<Vec<_>>>()?;
    let all_dead = convert(&opening.dead, model, count + model.tanks + 1)?;
    if model.dead_fraction == 0.0 && all_dead.iter().any(|value| *value != 0.0) {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "nonzero dead stock requires nonzero dead volume",
        ));
    }
    let dead = selected(&opening.dead)?;
    let tank_volume = quotient(
        product(1.0 - model.dead_fraction, model.volume_m3.value())?,
        model.tanks as f64,
    )?;
    let mut transitions = Vec::with_capacity(count);
    let mut steps = 0usize;
    let mut next = model.scope.interval.start_ns;
    for row in intervals {
        poll(cancelled)?;
        let value = &row.value;
        let t = value.scope.interval;
        if value.scope.boundary != model.scope.boundary
            || value.element != *element
            || t.start_ns != next
            || t.end_ns > model.scope.interval.end_ns
        {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "inverse observations must exactly tile the declared element/window",
            ));
        }
        let elapsed = t
            .end_ns
            .checked_sub(t.start_ns)
            .ok_or_else(|| numerical("inverse duration overflows"))? as f64
            * 1e-9;
        let volume = value.slurry_volume_m3.value();
        if !volume.is_finite() || volume < 0.0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "inverse slurry volume must be finite and nonnegative",
            ));
        }
        if volume == 0.0 && value.feed_kg.is_some_and(|q| q.signum() != 0) {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "nonzero observed feed requires slurry throughput",
            ));
        }
        let flow = quotient(volume, elapsed)?;
        if flow < model.total_flow_domain_m3_per_s[0].value()
            || flow > model.total_flow_domain_m3_per_s[1].value()
        {
            return Err(fail(
                RefusalCode::OutsideAoa203,
                "inverse interval-average flow is outside the declared domain",
            ));
        }
        let x = quotient(product(1.0 - model.bypass_fraction, volume)?, tank_volume)?;
        let step_count = if x == 0.0 {
            0.0
        } else {
            (x / kernel::MAX_TURNOVER).ceil().max(1.0)
        };
        if !step_count.is_finite() || step_count > ((model.max_kernel_steps - steps) / 2) as f64 {
            return Err(numerical(
                "inverse response plus replay exceeds declared kernel-step budget",
            ));
        }
        let step_count = step_count as usize;
        steps += 2 * step_count;
        let work = steps
            .checked_mul(model.tanks)
            .and_then(|x| x.checked_mul(model.tanks + 1))
            .and_then(|x| x.checked_mul(count + 2));
        if work.is_none_or(|work| work > MAX_RESPONSE_WORK) {
            return Err(numerical(
                "inverse response construction exceeds its bounded scalar-work budget",
            ));
        }
        transitions.push(Transition {
            count: step_count,
            weights: if step_count == 0 {
                None
            } else {
                Some(kernel::weights(
                    model.tanks,
                    quotient(x, step_count as f64)?,
                )?)
            },
        });
        next = t.end_ns;
    }
    if next != model.scope.interval.end_ns {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "inverse observations omit the final interval",
        ));
    }
    let mut stocks = initial_stocks
        .iter()
        .map(|&q| {
            let mut row = vec![0.0; count + 1];
            row[0] = q;
            row
        })
        .collect::<Vec<_>>();
    let mut matrix = Vec::with_capacity(count * count);
    let mut initial_outlet = Vec::with_capacity(count);
    for (t, transition) in transitions.iter().enumerate() {
        let mut outlet: Vec<_> = (0..=count).map(|_| Sum::default()).collect();
        if let Some(weights) = &transition.weights {
            let mut input = vec![0.0; count + 1];
            input[t + 1] = quotient(1.0 - model.bypass_fraction, transition.count as f64)?;
            for _ in 0..transition.count {
                poll(cancelled)?;
                for (acc, value) in
                    outlet
                        .iter_mut()
                        .zip(kernel::advance(&mut stocks, &input, weights)?)
                {
                    acc.add(value)?;
                }
            }
        }
        // With zero throughput the integrated feed is physically zero, including
        // a complete-bypass model. Its missing feed is therefore unobservable.
        if intervals[t].value.slurry_volume_m3.value() > 0.0 {
            outlet[t + 1].add(model.bypass_fraction)?;
        }
        initial_outlet.push(outlet[0].finish()?);
        matrix.extend(
            outlet[1..]
                .iter()
                .map(Sum::finish)
                .collect::<Result<Vec<_>>>()?,
        );
    }
    Ok(Response {
        matrix,
        initial_outlet,
        initial_stocks,
        dead,
        transitions,
        steps,
    })
}

struct Replay {
    outlet: Vec<f64>,
    inventory: Vec<f64>,
    conservation: Vec<f64>,
    whole_residual: f64,
}
fn replay(
    model: &RtdModel,
    response: &Response,
    feed: &[f64],
    cancelled: &mut impl FnMut() -> bool,
) -> Result<Replay> {
    let mut stock: Vec<_> = response.initial_stocks.iter().map(|q| vec![*q]).collect();
    let initial = sum(response
        .initial_stocks
        .iter()
        .copied()
        .chain([response.dead]))?;
    let mut previous = initial;
    let mut outlet = Vec::with_capacity(feed.len());
    let mut inventory = Vec::with_capacity(feed.len());
    let mut conservation = Vec::with_capacity(feed.len());
    for (transition, &incoming) in response.transitions.iter().zip(feed) {
        poll(cancelled)?;
        let mut discharged = Sum::default();
        discharged.add(product(model.bypass_fraction, incoming)?)?;
        if let Some(weights) = &transition.weights {
            let part = quotient(
                product(1.0 - model.bypass_fraction, incoming)?,
                transition.count as f64,
            )?;
            for _ in 0..transition.count {
                poll(cancelled)?;
                discharged.add(kernel::advance(&mut stock, &[part], weights)?[0])?;
            }
        }
        let out = discharged.finish()?;
        let closing = sum(stock.iter().map(|row| row[0]).chain([response.dead]))?;
        conservation.push(sum([previous, incoming, -out, -closing])?);
        outlet.push(out);
        inventory.push(closing);
        previous = closing;
    }
    let whole_residual = sum([
        initial,
        sum(feed.iter().copied())?,
        -sum(outlet.iter().copied())?,
        -previous,
    ])?;
    Ok(Replay {
        outlet,
        inventory,
        conservation,
        whole_residual,
    })
}

/// Jointly reconcile the noisy feed/outlet history under the declared fixed RTD.
/// A rank-deficient inverse refuses; an identifiable fit with no redundancy
/// explicitly supplies no gross-error test. Physical/gross-error failures retain
/// the complete diagnostic candidate but cannot yield an admitted Claim.
pub fn deconvolve(
    model: &Claim<RtdModel>,
    opening: &Claim<RtdOpening>,
    intervals: &[Claim<InverseInterval>],
    error_model: &Claim<InverseErrorModel>,
    mut cancelled: impl FnMut() -> bool,
) -> Result<InverseOutcome> {
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
    let rows = ordered(intervals)?;
    let observations = observation_order(intervals)?;
    let e = &error_model.value;
    let n = observations.len();
    let periods = rows.len();
    let p = 2 * periods - n;
    if e.observations.len() != n || e.covariance.len() != n * n {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "inverse covariance must cover every genuine observation and no unmeasured feed",
        ));
    }
    admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &e.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if e.scope != model.value.scope
        || e.element != rows[0].value.element
        || e.model_source != model.claim_id
        || e.opening_source != opening.claim_id
        || e.observations != observations
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "inverse error model must bind the exact kernel, opening and ordered observations",
        ));
    }
    if !named(&e.error_model_basis)
        || e.error_model_basis.len() > 4096
        || !e.fixed_transport_and_opening
        || !e.unbiased_gaussian_errors
        || !e.global_chi_square_limit.is_finite()
        || e.global_chi_square_limit <= 0.0
        || e.normalized_adjustment_limit
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        || !e.closure_tolerance_kg.value().is_finite()
        || e.closure_tolerance_kg.value() < 0.0
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "inverse requires a declared error basis, fixed kernel/opening, unbiased Gaussian errors and finite diagnostic limits",
        ));
    }
    let mut exact = Vec::with_capacity(n);
    let mut feed_indices = Vec::with_capacity(periods);
    let mut outlet_indices = Vec::with_capacity(periods);
    let mut missing = Vec::with_capacity(p);
    for row in &rows {
        if let Some(feed) = row.value.feed_kg {
            feed_indices.push(exact.len());
            exact.push(feed);
        } else {
            feed_indices.push(n + missing.len());
            missing.push(row.claim_id);
        }
        outlet_indices.push(exact.len());
        exact.push(row.value.outlet_kg);
    }
    let mut observed = Vec::with_capacity(n);
    let mut precision = Vec::with_capacity(n);
    for (i, &q) in exact.iter().enumerate() {
        let (value, budget) = nominal_with_precision_budget(
            q,
            e.covariance[i * n + i],
            e.closure_tolerance_kg,
            n + p,
        )?;
        observed.push(value);
        precision.push(budget);
    }
    let allowance = precision.iter().copied().fold(f64::INFINITY, f64::min);
    let response = response(
        &model.value,
        &opening.value,
        &rows,
        &e.element,
        allowance,
        &mut cancelled,
    )?;
    let mut measured = vec![0.0; periods * n];
    let mut latent = vec![0.0; periods * p];
    for t in 0..periods {
        measured[t * n + outlet_indices[t]] = 1.0;
        for (j, &coordinate) in feed_indices.iter().enumerate() {
            let coefficient = -response.matrix[t * periods + j];
            if coordinate < n {
                measured[t * n + coordinate] = coefficient;
            } else {
                latent[t * p + coordinate - n] = coefficient;
            }
        }
    }
    let covariance: Vec<_> = e.covariance.iter().map(|q| q.value()).collect();
    let fit = if p == 0 {
        let fit = reconcile_linear(&observed, &covariance, &measured, &response.initial_outlet)?;
        PartialReconciliation {
            measured_estimates: fit.estimates,
            measured_adjustments: fit.adjustments,
            latent_estimates: vec![],
            posterior_covariance: fit.posterior_covariance,
            adjustment_covariance: fit.adjustment_covariance,
            raw_observed_residuals: fit.raw_residuals,
            reconciled_residuals: fit.reconciled_residuals,
            chi_square: fit.chi_square,
            degrees_of_freedom: fit.degrees_of_freedom,
            normalized_adjustments: fit.normalized_adjustments,
            estimator_map: vec![],
        }
    } else {
        reconcile_with_unmeasured(
            &observed,
            &covariance,
            &measured,
            &latent,
            &response.initial_outlet,
            p,
        )?
    };
    poll(&mut cancelled)?;
    for i in 0..n {
        if ((fit.measured_estimates[i] - observed[i]) - fit.measured_adjustments[i]).abs()
            > precision[i]
        {
            return Err(numerical(
                "inverse fitted observation loses its declared adjustment precision",
            ));
        }
    }
    let fitted: Vec<_> = fit
        .measured_estimates
        .iter()
        .chain(&fit.latent_estimates)
        .copied()
        .collect();
    let feed: Vec<_> = feed_indices.iter().map(|&i| fitted[i]).collect();
    let out: Vec<_> = outlet_indices.iter().map(|&i| fitted[i]).collect();
    let replay = replay(&model.value, &response, &feed, &mut cancelled)?;
    let transport_residual = replay
        .outlet
        .iter()
        .zip(&out)
        .map(|(&a, &b)| sum([a, -b]))
        .collect::<Result<Vec<_>>>()?;
    let tolerance = e.closure_tolerance_kg.value();
    let rejection = if fitted.iter().chain(&replay.inventory).any(|v| *v < 0.0)
        || rows
            .iter()
            .zip(&feed)
            .any(|(row, feed)| row.value.slurry_volume_m3.value() == 0.0 && *feed != 0.0)
    {
        Some(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "inverse fitted feed, outlet or inventory is physically inadmissible; no negative mass is clamped",
        ))
    } else if transport_residual
        .iter()
        .chain(&replay.conservation)
        .chain([&replay.whole_residual])
        .chain(&fit.reconciled_residuals)
        .any(|v| v.abs() > tolerance)
    {
        Some(fail(
            RefusalCode::Conservation101,
            "inverse forward replay or conservation exceeds the declared kg allowance",
        ))
    } else if fit.degrees_of_freedom > 0 && fit.chi_square > e.global_chi_square_limit {
        Some(fail(
            RefusalCode::Conservation101,
            "inverse joint gross-error statistic exceeds the declared limit; every observation is retained",
        ))
    } else if fit.degrees_of_freedom > 0
        && e.normalized_adjustment_limit.is_some_and(|limit| {
            fit.normalized_adjustments
                .iter()
                .flatten()
                .any(|v| v.abs() > limit)
        })
    {
        Some(fail(
            RefusalCode::Conservation101,
            "inverse normalized adjustments exceed the declared simultaneous policy",
        ))
    } else {
        None
    };
    let gross_error = if fit.degrees_of_freedom == 0 {
        GrossErrorDiagnostic::NoRedundancy
    } else {
        GrossErrorDiagnostic::Available {
            chi_square: fit.chi_square,
            degrees_of_freedom: fit.degrees_of_freedom,
        }
    };
    let masses = |values: Vec<f64>| values.into_iter().map(Kilograms::new).collect();
    let diagnostics = InverseReconciliation {
        scope: e.scope.clone(),
        element: e.element.clone(),
        observations,
        unmeasured_feeds: missing,
        observed_kg: masses(observed),
        estimates_kg: masses(fitted),
        measured_adjustments_kg: masses(fit.measured_adjustments),
        posterior_covariance: fit
            .posterior_covariance
            .into_iter()
            .map(KilogramsSquared::new)
            .collect(),
        measured_adjustment_covariance: fit
            .adjustment_covariance
            .into_iter()
            .map(KilogramsSquared::new)
            .collect(),
        normalized_adjustments: fit.normalized_adjustments,
        feed_coordinate_indices: feed_indices,
        outlet_coordinate_indices: outlet_indices,
        response_matrix: response.matrix,
        opening_outlet_kg: masses(response.initial_outlet),
        fitted_feed_kg: masses(feed),
        fitted_outlet_kg: masses(out),
        predicted_outlet_kg: masses(replay.outlet),
        closing_inventory_kg: masses(replay.inventory),
        raw_observed_residual_kg: masses(fit.raw_observed_residuals),
        transport_residual_kg: masses(transport_residual),
        conservation_residual_kg: masses(replay.conservation),
        whole_window_residual_kg: Kilograms::new(replay.whole_residual),
        gross_error,
        kernel_steps: response.steps,
    };
    let mut metadata = Metadata::new();
    let mut enc = Encoder::new_frame(*b"RTIR", 1);
    put_source_envelope(&mut enc, model, 0);
    metadata.include(model)?;
    put_source_envelope(&mut enc, opening, 1);
    metadata.include(opening)?;
    enc.put_u64(rows.len() as u64);
    for row in &rows {
        put_source_envelope(&mut enc, *row, 2);
        metadata.include(*row)?;
    }
    put_source_envelope(&mut enc, error_model, 3);
    metadata.include(error_model)?;
    put_diagnostics(&mut enc, &diagnostics);
    enc.put_u8(u8::from(rejection.is_some()));
    if let Some(refusal) = &rejection {
        enc.put_str(refusal.code);
        enc.put_str(&refusal.what);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    poll(&mut cancelled)?;
    let intervals = rows.into_iter().cloned().collect();
    if let Some(refusal) = rejection {
        Ok(InverseOutcome::Rejected(Box::new(RejectedInverse {
            model: model.clone(),
            opening: opening.clone(),
            intervals,
            error_model: error_model.clone(),
            diagnostics,
            refusal,
            digest,
        })))
    } else {
        let claim = metadata.into_claim(OUTPUT_METRIC, diagnostics, &digest);
        Ok(InverseOutcome::Admitted(Box::new(InverseReceipt {
            model: model.clone(),
            opening: opening.clone(),
            intervals,
            error_model: error_model.clone(),
            claim,
            digest,
        })))
    }
}

fn put_diagnostics(enc: &mut Encoder, value: &InverseReconciliation) {
    for values in [
        &value.observed_kg,
        &value.estimates_kg,
        &value.measured_adjustments_kg,
        &value.opening_outlet_kg,
        &value.fitted_feed_kg,
        &value.fitted_outlet_kg,
        &value.predicted_outlet_kg,
        &value.closing_inventory_kg,
        &value.raw_observed_residual_kg,
        &value.transport_residual_kg,
        &value.conservation_residual_kg,
    ] {
        enc.put_u64(values.len() as u64);
        for q in values {
            enc.put_u64(q.value().to_bits());
        }
    }
    for values in [
        &value.posterior_covariance,
        &value.measured_adjustment_covariance,
    ] {
        enc.put_u64(values.len() as u64);
        for q in values {
            enc.put_u64(q.value().to_bits());
        }
    }
    enc.put_u64(value.normalized_adjustments.len() as u64);
    for &v in &value.normalized_adjustments {
        put_optional(enc, v);
    }
    for indices in [
        &value.feed_coordinate_indices,
        &value.outlet_coordinate_indices,
    ] {
        enc.put_u64(indices.len() as u64);
        for &i in indices {
            enc.put_u64(i as u64);
        }
    }
    enc.put_u64(value.response_matrix.len() as u64);
    for v in &value.response_matrix {
        enc.put_u64(v.to_bits());
    }
    enc.put_u64(value.whole_window_residual_kg.value().to_bits());
    enc.put_u64(value.kernel_steps as u64);
    match value.gross_error {
        GrossErrorDiagnostic::NoRedundancy => enc.put_u8(0),
        GrossErrorDiagnostic::Available {
            chi_square,
            degrees_of_freedom,
        } => {
            enc.put_u8(1);
            enc.put_u64(chi_square.to_bits());
            enc.put_u64(degrees_of_freedom as u64);
        }
    }
}

#[cfg(test)]
mod tests;
