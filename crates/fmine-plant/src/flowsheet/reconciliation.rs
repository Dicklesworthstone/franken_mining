//! DESIGN: joint R1 reconciliation of a complete conservative plant graph
//! (plan §§17.5, 20.2–20.3; BD-149/175/177). Every measured stream and stock has
//! one coordinate per carrier/element. An internal stream uses the SAME
//! coordinate at both endpoints. Only unit balances constrain the fit; adding
//! the whole-boundary balance would duplicate their sum. All uncertainty is
//! supplied in one source-bound covariance, including cross-stream and
//! cross-element correlations. Missing observations, bias estimation, reaction
//! stoichiometry and process prediction are outside this conditional model.
//! Exact nominal observations survive unchanged. Fitted binary64 values never
//! become an exact accounting receipt, and a rejected fit has no balance Claim.
//! Determinism: DeterministicPerIsa

use super::{
    AdmittedFlowsheet, ElementKey, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts,
    Rows, StocktakeKind, StreamId, UnitId, admit_flowsheet, put_element, put_qty, put_time,
    validate_amounts,
};
use crate::{
    inventory,
    reconciliation::{
        Kilograms, KilogramsSquared, Metadata, nominal_with_precision_budget, represented_rational,
    },
};
use fmine_claim::{AdditivityClass, Claim, ClaimId};
#[cfg(test)]
use fmine_claim::{Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
#[cfg(test)]
use fmine_evidence::KnowledgeState;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::{MAX_CONSTRAINTS, MAX_OBSERVATIONS, reconcile_linear};
use fmine_units::ExactQty;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Refusal>;
pub const ERROR_MODEL_METRIC: &str = "plant.flowsheet.error_model/1";
pub const OUTPUT_METRIC: &str = "plant.flowsheet.reconciled_interval/1";

/// Carriers overlap the elemental coordinates; they are not added together.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MaterialChannel {
    DrySolids,
    Water,
    Element(ElementKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObservationLocation {
    Transfer(StreamId),
    Stocktake { unit: UnitId, kind: StocktakeKind },
}

/// Identity of one coordinate in the covariance and fitted vectors.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservationKey {
    pub source: ClaimId,
    pub location: ObservationLocation,
    pub channel: MaterialChannel,
}

/// Complete source-declared observation model. Its deterministic envelope
/// describes explicitly supplied parameters; it does not erase the random
/// errors represented by the covariance inside it.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetErrorModel {
    pub scope: FlowsheetScope,
    pub declaration_id: ClaimId,
    /// Must exactly equal `observation_order`, including the declared channels.
    pub observations: Vec<ObservationKey>,
    /// Full n×n row-major positive-definite covariance, in kg². No independence
    /// between streams, stocktakes or different elements is inferred.
    pub covariance: Vec<KilogramsSquared>,
    pub unbiased_gaussian_errors: bool,
    pub global_chi_square_limit: f64,
    /// An explicit simultaneous diagnostic policy; individual adjustments are
    /// correlated and do not identify a uniquely faulty instrument.
    pub normalized_adjustment_limit: Option<f64>,
    /// Exactly one nonnegative allowance for EVERY carrier/element, in kg.
    /// These are domain allowances, separate from the numerical owner's audit.
    pub closure_tolerances_kg: BTreeMap<MaterialChannel, Kilograms>,
}
impl FlowsheetErrorModel {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"FGEM", 1);
        enc.put_str(&self.scope.boundary);
        put_time(&mut enc, self.scope.interval);
        enc.put_str(&self.declaration_id.to_string());
        enc.put_u64(self.observations.len() as u64);
        for key in &self.observations {
            put_key(&mut enc, key);
        }
        enc.put_u64(self.covariance.len() as u64);
        for value in &self.covariance {
            enc.put_u64(value.value().to_bits());
        }
        enc.put_u8(u8::from(self.unbiased_gaussian_errors));
        enc.put_u64(self.global_chi_square_limit.to_bits());
        put_optional(&mut enc, self.normalized_adjustment_limit);
        enc.put_u64(self.closure_tolerances_kg.len() as u64);
        for (channel, tolerance) in &self.closure_tolerances_kg {
            put_channel(&mut enc, channel);
            enc.put_u64(tolerance.value().to_bits());
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// A unit/channel conservation row, in canonical unit then channel order.
#[derive(Debug, Clone, PartialEq)]
pub struct ConservationDiagnostic {
    pub unit: UnitId,
    pub channel: MaterialChannel,
    pub raw_residual_exact_kg: ExactQty,
    pub raw_residual_kg: Kilograms,
    pub reconciled_residual_kg: Kilograms,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryRole {
    OpeningInventory,
    Feed,
    Product,
    OtherOutflow,
    ClosingInventory,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundaryKey {
    pub role: BoundaryRole,
    pub channel: MaterialChannel,
}

/// Linear sums over external streams and all unit stocks. Internal circulation
/// is excluded. Declared roles without incident streams are structural zeros.
/// The complete covariance must accompany these correlated role totals.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryProjection {
    pub keys: Vec<BoundaryKey>,
    pub observed_kg: Vec<Kilograms>,
    pub estimates_kg: Vec<Kilograms>,
    pub posterior_covariance: Vec<KilogramsSquared>,
    /// Independent audit of projected totals, not additional solver rows.
    pub conservation: Vec<BoundaryConservationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryConservationDiagnostic {
    pub channel: MaterialChannel,
    pub raw_residual_exact_kg: ExactQty,
    pub raw_residual_kg: Kilograms,
    pub reconciled_residual_kg: Kilograms,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetReconciliation {
    pub scope: FlowsheetScope,
    pub observations: Vec<ObservationKey>,
    pub observed_kg: Vec<Kilograms>,
    pub estimates_kg: Vec<Kilograms>,
    pub adjustments_kg: Vec<Kilograms>,
    /// Both matrices are full n×n and follow `observations` exactly.
    pub posterior_covariance: Vec<KilogramsSquared>,
    pub adjustment_covariance: Vec<KilogramsSquared>,
    pub normalized_adjustments: Vec<Option<f64>>,
    pub conservation: Vec<ConservationDiagnostic>,
    pub boundary: BoundaryProjection,
    pub chi_square: f64,
    pub degrees_of_freedom: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciledFlowsheetReceipt {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<FlowsheetErrorModel>,
    claim: Claim<FlowsheetReconciliation>,
    digest: String,
}
impl ReconciledFlowsheetReceipt {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<FlowsheetErrorModel> {
        &self.error_model
    }
    pub fn claim(&self) -> &Claim<FlowsheetReconciliation> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RejectedFlowsheetReconciliation {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<FlowsheetErrorModel>,
    diagnostics: FlowsheetReconciliation,
    refusal: Refusal,
    digest: String,
}
impl RejectedFlowsheetReconciliation {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<FlowsheetErrorModel> {
        &self.error_model
    }
    pub fn diagnostics(&self) -> &FlowsheetReconciliation {
        &self.diagnostics
    }
    pub fn refusal(&self) -> &Refusal {
        &self.refusal
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowsheetReconciliationOutcome {
    Admitted(Box<ReconciledFlowsheetReceipt>),
    Rejected(Box<RejectedFlowsheetReconciliation>),
}
impl FlowsheetReconciliationOutcome {
    pub fn diagnostics(&self) -> &FlowsheetReconciliation {
        match self {
            Self::Admitted(r) => &r.claim.value,
            Self::Rejected(r) => &r.diagnostics,
        }
    }
    pub fn admitted_claim(&self) -> Result<&Claim<FlowsheetReconciliation>> {
        match self {
            Self::Admitted(r) => Ok(&r.claim),
            Self::Rejected(r) => Err(r.refusal.clone()),
        }
    }
}

fn fail(code: RefusalCode, message: impl Into<String>) -> Refusal {
    Refusal::new(
        code,
        format!("flowsheet reconciliation: {}", message.into()),
    )
}
fn numerical(message: impl Into<String>) -> Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, message)
}
fn channels(graph: &AdmittedFlowsheet<'_>) -> Vec<MaterialChannel> {
    let mut channels = vec![MaterialChannel::DrySolids, MaterialChannel::Water];
    channels.extend(graph.schema.iter().cloned().map(MaterialChannel::Element));
    channels
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
fn put_key(enc: &mut Encoder, key: &ObservationKey) {
    enc.put_str(&key.source.to_string());
    match &key.location {
        ObservationLocation::Transfer(stream) => {
            enc.put_u8(0);
            enc.put_str(stream.as_str());
        }
        ObservationLocation::Stocktake { unit, kind } => {
            enc.put_u8(1);
            enc.put_str(unit.as_str());
            enc.put_u8(match kind {
                StocktakeKind::Opening => 0,
                StocktakeKind::Closing => 1,
            });
        }
    }
    put_channel(enc, &key.channel);
}
fn put_optional(enc: &mut Encoder, value: Option<f64>) {
    enc.put_u8(u8::from(value.is_some()));
    if let Some(value) = value {
        enc.put_u64(value.to_bits());
    }
}
fn amount(amounts: &MaterialAmounts, channel: &MaterialChannel) -> Result<ExactQty> {
    match channel {
        MaterialChannel::DrySolids => Ok(amounts.dry_solids_kg),
        MaterialChannel::Water => Ok(amounts.water_kg),
        MaterialChannel::Element(element) => amounts
            .elements_kg
            .get(element)
            .copied()
            .ok_or_else(|| fail(RefusalCode::BasisMismatch105, "missing admitted element")),
    }
}

struct Coordinates {
    channels: Vec<MaterialChannel>,
    keys: Vec<ObservationKey>,
    exact: Vec<ExactQty>,
    offsets: BTreeMap<ClaimId, usize>,
}
fn coordinates(graph: &AdmittedFlowsheet<'_>) -> Result<Coordinates> {
    let channels = channels(graph);
    let source_count = graph.inputs.len() - 1;
    let n = source_count
        .checked_mul(channels.len())
        .ok_or_else(|| numerical("coordinate count overflow"))?;
    let m = graph
        .per_unit
        .len()
        .checked_mul(channels.len())
        .ok_or_else(|| numerical("constraint count overflow"))?;
    if n == 0 || n > MAX_OBSERVATIONS || m > MAX_CONSTRAINTS {
        return Err(numerical(
            "graph exceeds the complete-data numerical work limit",
        ));
    }
    let mut keys = Vec::with_capacity(n);
    let mut exact = Vec::with_capacity(n);
    let mut offsets = BTreeMap::new();
    for source in &graph.inputs {
        let location = match &source.value.value {
            FlowsheetValue::Declaration { .. } => continue,
            FlowsheetValue::Transfer { stream, .. } => {
                ObservationLocation::Transfer(stream.clone())
            }
            FlowsheetValue::Stocktake { unit, kind, .. } => ObservationLocation::Stocktake {
                unit: unit.clone(),
                kind: *kind,
            },
        };
        if offsets.insert(source.claim_id, keys.len()).is_some() {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "one source identity appears in multiple observation roles",
            ));
        }
        let amounts = source
            .value
            .value
            .amounts()
            .ok_or_else(|| numerical("missing admitted amount vector"))?;
        for channel in &channels {
            keys.push(ObservationKey {
                source: source.claim_id,
                location: location.clone(),
                channel: channel.clone(),
            });
            exact.push(amount(amounts, channel)?);
        }
    }
    Ok(Coordinates {
        channels,
        keys,
        exact,
        offsets,
    })
}

/// Return the ONLY admitted covariance ordering, after source/topology admission
/// and before any closure test. The caller must explicitly supply the error
/// model; this helper supplies neither variances nor Gaussian assumptions.
pub fn observation_order(inputs: &[Claim<FlowsheetInput>]) -> Result<Vec<ObservationKey>> {
    Ok(coordinates(&admit_flowsheet(inputs)?)?.keys)
}

fn role_rows<'rows, 'source>(
    rows: &'rows Rows<'source>,
) -> [(BoundaryRole, &'rows [super::Source<'source>]); 5] {
    [
        (BoundaryRole::OpeningInventory, &rows.opening),
        (BoundaryRole::Feed, &rows.feed),
        (BoundaryRole::Product, &rows.product),
        (BoundaryRole::OtherOutflow, &rows.other),
        (BoundaryRole::ClosingInventory, &rows.closing),
    ]
}
fn positive(role: BoundaryRole) -> bool {
    matches!(role, BoundaryRole::OpeningInventory | BoundaryRole::Feed)
}
fn offset(data: &Coordinates, source: ClaimId) -> Result<usize> {
    data.offsets
        .get(&source)
        .copied()
        .ok_or_else(|| numerical("missing admitted coordinate"))
}

fn constraints(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
) -> Result<(Vec<f64>, Vec<ConservationDiagnostic>)> {
    let n = data.keys.len();
    let mut matrix = Vec::new();
    let mut diagnostics = Vec::new();
    for (unit, rows) in &graph.per_unit {
        for (c, channel) in data.channels.iter().enumerate() {
            let mut row = vec![0.0; n];
            let mut raw_exact = ExactQty::from_integer(0);
            for (role, sources) in role_rows(rows) {
                for source in sources {
                    let i = offset(data, source.claim_id)? + c;
                    row[i] = if positive(role) { 1.0 } else { -1.0 };
                    raw_exact = if positive(role) {
                        raw_exact.checked_add(data.exact[i])
                    } else {
                        raw_exact.checked_sub(data.exact[i])
                    }
                    .map_err(|_| numerical("exact raw unit balance overflow"))?;
                }
            }
            matrix.extend(row);
            diagnostics.push(ConservationDiagnostic {
                unit: unit.clone(),
                channel: channel.clone(),
                raw_residual_exact_kg: raw_exact,
                raw_residual_kg: Kilograms::new(0.0),
                reconciled_residual_kg: Kilograms::new(0.0),
            });
        }
    }
    Ok((matrix, diagnostics))
}

// Linear aggregation of an already certified covariance; unlike raw error-model
// admission, a posterior is necessarily singular. Only coefficient-one sums
// are needed here. Compensated arithmetic and consistency checks never clip.
fn sum(values: impl Iterator<Item = f64>) -> Result<f64> {
    let (mut total, mut correction) = (0.0_f64, 0.0_f64);
    for value in values {
        let next = total + value;
        correction += if total.abs() >= value.abs() {
            (total - next) + value
        } else {
            (value - next) + total
        };
        total = next;
    }
    let value = total + correction;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(numerical("boundary projection overflow"))
    }
}
fn boundary_projection(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    observed: &[f64],
    fitted: &[f64],
    covariance: &[f64],
    tolerances: &BTreeMap<MaterialChannel, Kilograms>,
) -> Result<BoundaryProjection> {
    let n = data.keys.len();
    let mut keys = Vec::new();
    let mut groups = Vec::new();
    for (role, sources) in role_rows(&graph.outer) {
        for (c, channel) in data.channels.iter().enumerate() {
            keys.push(BoundaryKey {
                role,
                channel: channel.clone(),
            });
            groups.push(
                sources
                    .iter()
                    .map(|source| offset(data, source.claim_id).map(|i| i + c))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    let k = groups.len();
    let observed_kg = groups
        .iter()
        .map(|group| sum(group.iter().map(|&i| observed[i])).map(Kilograms::new))
        .collect::<Result<Vec<_>>>()?;
    let estimates_kg = groups
        .iter()
        .map(|group| sum(group.iter().map(|&i| fitted[i])).map(Kilograms::new))
        .collect::<Result<Vec<_>>>()?;
    let mut posterior_covariance = vec![KilogramsSquared::new(0.0); k * k];
    for r in 0..k {
        for s in r..k {
            let value = sum(groups[r]
                .iter()
                .flat_map(|&i| groups[s].iter().map(move |&j| covariance[i * n + j])))?;
            posterior_covariance[r * k + s] = KilogramsSquared::new(value);
            posterior_covariance[s * k + r] = KilogramsSquared::new(value);
        }
        if posterior_covariance[r * k + r].value() < 0.0 {
            return Err(numerical("negative projected boundary variance"));
        }
    }
    for r in 0..k {
        for s in r + 1..k {
            let a = posterior_covariance[r * k + r].value().sqrt();
            let b = posterior_covariance[s * k + s].value().sqrt();
            let cross = posterior_covariance[r * k + s].value();
            if if a == 0.0 || b == 0.0 {
                cross != 0.0
            } else {
                ((cross / a.max(b)) / a.min(b)).abs() > 1.0 + 2e-10
            } {
                return Err(numerical("inconsistent projected boundary covariance"));
            }
        }
    }
    let mut conservation = Vec::new();
    for (c, channel) in data.channels.iter().enumerate() {
        let mut raw_exact = ExactQty::from_integer(0);
        let mut raw_terms = Vec::new();
        let mut fitted_terms = Vec::new();
        for role in 0..5 {
            let r = role * data.channels.len() + c;
            let sign = if positive(keys[r].role) { 1.0 } else { -1.0 };
            raw_terms.push(sign * observed_kg[r].value());
            fitted_terms.push(sign * estimates_kg[r].value());
            for &i in &groups[r] {
                raw_exact = if sign > 0.0 {
                    raw_exact.checked_add(data.exact[i])
                } else {
                    raw_exact.checked_sub(data.exact[i])
                }
                .map_err(|_| numerical("exact raw boundary balance overflow"))?;
            }
        }
        let raw = sum(raw_terms.into_iter())?;
        if (raw - raw_exact.to_f64()).abs() > tolerances[channel].value() {
            return Err(numerical(
                "projected totals lose the original exact boundary imbalance",
            ));
        }
        conservation.push(BoundaryConservationDiagnostic {
            channel: channel.clone(),
            raw_residual_exact_kg: raw_exact,
            raw_residual_kg: Kilograms::new(raw),
            reconciled_residual_kg: Kilograms::new(sum(fitted_terms.into_iter())?),
        });
    }
    Ok(BoundaryProjection {
        keys,
        observed_kg,
        estimates_kg,
        posterior_covariance,
        conservation,
    })
}

// Exact binary representations are used only to assess inequalities. They are
// NOT rounded physical measurements and never enter `account_flowsheet`.
fn physical_rejection(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    estimates: &[f64],
) -> Result<Option<Refusal>> {
    if let Some(i) = estimates.iter().position(|value| *value < 0.0) {
        return Ok(Some(fail(
            RefusalCode::OutsidePhysicalDomain109,
            format!("negative fitted mass at {:?}; no clipping", data.keys[i]),
        )));
    }
    for &start in data.offsets.values() {
        let mut amounts = MaterialAmounts {
            dry_solids_kg: represented_rational(estimates[start])?,
            water_kg: represented_rational(estimates[start + 1])?,
            elements_kg: BTreeMap::new(),
        };
        for (c, channel) in data.channels.iter().enumerate().skip(2) {
            if let MaterialChannel::Element(element) = channel {
                amounts
                    .elements_kg
                    .insert(element.clone(), represented_rational(estimates[start + c])?);
            }
        }
        if let Err(mut refusal) = validate_amounts(&amounts, &graph.schema) {
            refusal.what = format!(
                "fitted composition at {:?}: {}",
                data.keys[start].location, refusal.what
            );
            return Ok(Some(refusal));
        }
    }
    Ok(None)
}

/// Jointly reconcile every admitted stream/stock and carrier/element. Complete
/// source admission is shared with exact accounting, but a nonzero observed
/// balance is allowed into WLS. Rejection preserves the full calculation and
/// every observation; neither missing values nor balancing plugs are supplied.
pub fn reconcile_flowsheet(
    inputs: &[Claim<FlowsheetInput>],
    error_model: &Claim<FlowsheetErrorModel>,
) -> Result<FlowsheetReconciliationOutcome> {
    let graph = admit_flowsheet(inputs)?;
    let data = coordinates(&graph)?;
    let model = &error_model.value;
    let n = data.keys.len();
    inventory::admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if model.scope != graph.declaration.value.scope
        || model.declaration_id != graph.declaration.claim_id
        || model.observations != data.keys
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "error model must bind this declaration and every canonical observation coordinate",
        ));
    }
    if model.covariance.len() != n * n
        || !model.closure_tolerances_kg.keys().eq(data.channels.iter())
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "supply a full covariance and exactly one closure allowance per channel",
        ));
    }
    if !model.unbiased_gaussian_errors
        || !model.global_chi_square_limit.is_finite()
        || model.global_chi_square_limit <= 0.0
        || model
            .normalized_adjustment_limit
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        || model
            .closure_tolerances_kg
            .values()
            .any(|v| !v.value().is_finite() || v.value() < 0.0)
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "declare unbiased Gaussian errors, positive finite test limits and nonnegative finite kg allowances",
        ));
    }
    let mut observed = Vec::with_capacity(n);
    let mut allowance = Vec::with_capacity(n);
    for (i, q) in data.exact.iter().enumerate() {
        let (value, budget) = nominal_with_precision_budget(
            *q,
            model.covariance[i * n + i],
            model.closure_tolerances_kg[&data.keys[i].channel],
            data.offsets.len(),
        )?;
        observed.push(value);
        allowance.push(budget);
    }
    let (matrix, mut conservation) = constraints(&graph, &data)?;
    let covariance: Vec<_> = model.covariance.iter().map(|v| v.value()).collect();
    let result = reconcile_linear(
        &observed,
        &covariance,
        &matrix,
        &vec![0.0; conservation.len()],
    )?;
    for (r, diagnostic) in conservation.iter_mut().enumerate() {
        let tolerance = model.closure_tolerances_kg[&diagnostic.channel].value();
        if (result.raw_residuals[r] - diagnostic.raw_residual_exact_kg.to_f64()).abs() > tolerance {
            return Err(numerical(
                "binary64 raw residual loses an original exact unit imbalance",
            ));
        }
        diagnostic.raw_residual_kg = Kilograms::new(result.raw_residuals[r]);
        diagnostic.reconciled_residual_kg = Kilograms::new(result.reconciled_residuals[r]);
    }
    for i in 0..n {
        if ((result.estimates[i] - observed[i]) - result.adjustments[i]).abs() > allowance[i] {
            return Err(numerical(
                "fitted estimate cannot represent its adjustment within the declared numerical budget",
            ));
        }
    }
    let mut meta = Metadata::new();
    let mut enc = Encoder::new_frame(*b"FGRR", 1);
    enc.put_u64(graph.inputs.len() as u64);
    for source in &graph.inputs {
        inventory::put_source_envelope(&mut enc, *source, source.value.value.key().0);
        meta.include(*source)?;
    }
    inventory::put_source_envelope(&mut enc, error_model, 3);
    meta.include(error_model)?;
    let mut rejection = physical_rejection(&graph, &data, &result.estimates)?;
    if rejection.is_none()
        && conservation.iter().any(|d| {
            d.reconciled_residual_kg.value().abs() > model.closure_tolerances_kg[&d.channel].value()
        })
    {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            "fitted unit residual exceeds its declared channel allowance",
        ));
    }
    if rejection.is_none() && result.chi_square > model.global_chi_square_limit {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            format!(
                "global chi-square {} exceeds declared limit {}; all observations retained",
                result.chi_square, model.global_chi_square_limit
            ),
        ));
    }
    if rejection.is_none()
        && let Some(limit) = model.normalized_adjustment_limit
        && result
            .normalized_adjustments
            .iter()
            .flatten()
            .any(|value| value.abs() > limit)
    {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            format!(
                "normalized adjustment exceeds declared limit {limit}; correlated diagnostics are not unique fault attribution"
            ),
        ));
    }
    let boundary = boundary_projection(
        &graph,
        &data,
        &observed,
        &result.estimates,
        &result.posterior_covariance,
        &model.closure_tolerances_kg,
    )?;
    if rejection.is_none()
        && boundary.conservation.iter().any(|d| {
            d.reconciled_residual_kg.value().abs() > model.closure_tolerances_kg[&d.channel].value()
        })
    {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            "projected whole-boundary residual exceeds its declared channel allowance",
        ));
    }
    let diagnostics = FlowsheetReconciliation {
        scope: model.scope.clone(),
        observations: data.keys,
        observed_kg: observed.into_iter().map(Kilograms::new).collect(),
        estimates_kg: result
            .estimates
            .iter()
            .copied()
            .map(Kilograms::new)
            .collect(),
        adjustments_kg: result
            .adjustments
            .iter()
            .copied()
            .map(Kilograms::new)
            .collect(),
        posterior_covariance: result
            .posterior_covariance
            .iter()
            .copied()
            .map(KilogramsSquared::new)
            .collect(),
        adjustment_covariance: result
            .adjustment_covariance
            .iter()
            .copied()
            .map(KilogramsSquared::new)
            .collect(),
        normalized_adjustments: result.normalized_adjustments,
        conservation,
        boundary,
        chi_square: result.chi_square,
        degrees_of_freedom: result.degrees_of_freedom,
    };
    put_diagnostics(&mut enc, &diagnostics);
    enc.put_u8(u8::from(rejection.is_some()));
    if let Some(refusal) = &rejection {
        enc.put_str(refusal.code);
        enc.put_str(&refusal.what);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let inputs = graph.inputs.into_iter().cloned().collect();
    if let Some(refusal) = rejection {
        return Ok(FlowsheetReconciliationOutcome::Rejected(Box::new(
            RejectedFlowsheetReconciliation {
                inputs,
                error_model: error_model.clone(),
                diagnostics,
                refusal,
                digest,
            },
        )));
    }
    let claim = meta.into_claim(OUTPUT_METRIC, diagnostics, &digest);
    Ok(FlowsheetReconciliationOutcome::Admitted(Box::new(
        ReconciledFlowsheetReceipt {
            inputs,
            error_model: error_model.clone(),
            claim,
            digest,
        },
    )))
}

fn put_diagnostics(enc: &mut Encoder, d: &FlowsheetReconciliation) {
    for key in &d.observations {
        put_key(enc, key);
    }
    for values in [&d.observed_kg, &d.estimates_kg, &d.adjustments_kg] {
        for value in values {
            enc.put_u64(value.value().to_bits());
        }
    }
    for values in [&d.posterior_covariance, &d.adjustment_covariance] {
        for value in values {
            enc.put_u64(value.value().to_bits());
        }
    }
    for value in &d.normalized_adjustments {
        put_optional(enc, *value);
    }
    for row in &d.conservation {
        enc.put_str(row.unit.as_str());
        put_channel(enc, &row.channel);
        put_qty(enc, row.raw_residual_exact_kg);
        enc.put_u64(row.raw_residual_kg.value().to_bits());
        enc.put_u64(row.reconciled_residual_kg.value().to_bits());
    }
    for key in &d.boundary.keys {
        enc.put_u8(key.role as u8);
        put_channel(enc, &key.channel);
    }
    for values in [&d.boundary.observed_kg, &d.boundary.estimates_kg] {
        for value in values {
            enc.put_u64(value.value().to_bits());
        }
    }
    for value in &d.boundary.posterior_covariance {
        enc.put_u64(value.value().to_bits());
    }
    for row in &d.boundary.conservation {
        put_channel(enc, &row.channel);
        put_qty(enc, row.raw_residual_exact_kg);
        enc.put_u64(row.raw_residual_kg.value().to_bits());
        enc.put_u64(row.reconciled_residual_kg.value().to_bits());
    }
    enc.put_u64(d.chi_square.to_bits());
    enc.put_u64(d.degrees_of_freedom as u64);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod joint_oracles;
