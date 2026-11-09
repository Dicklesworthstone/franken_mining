//! DESIGN: infer explicitly unmeasured extensive quantities from observed stocks
//! and flows (plan §§20.2–20.3 R1, BD-149/154/177). The original flow-only API
//! retains both stocktakes; `quantities` also admits one unknown endpoint per unit.
//! The shared numerical owner solves measured/latent GLS and refuses deficient
//! observability. No observation, variance, prior, balancing plug or zero is
//! invented for a missing quantity. This is not a temporal/bias R2 model.
//! Determinism: DeterministicPerIsa

use super::{
    BoundaryKey, BoundaryRole, Coordinates, MaterialChannel, ObservationKey, ObservationLocation,
    constraints, coordinates, fail, numerical, offset, physical_rejection, positive,
    project_group_covariance, put_channel, put_key, put_optional, role_rows, sum,
};
use crate::flowsheet::{
    AdmittedFlowsheet, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts,
    StocktakeKind, StreamId, StreamRoute, UnitId, admit_flowsheet_with_unmeasured,
    admit_flowsheet_with_unmeasured_quantities, put_qty, put_time, validate_amounts,
};
use crate::{
    inventory,
    reconciliation::{
        Kilograms, KilogramsSquared, Metadata, nominal_with_precision_budget, represented_rational,
    },
};
use fmine_claim::{AdditivityClass, Claim, ClaimId};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::{MAX_OBSERVATIONS, partial::reconcile_with_unmeasured};
use fmine_units::ExactQty;
use std::collections::{BTreeMap, BTreeSet};

pub mod quantities;
use quantities::UnmeasuredQuantityKey;

type Result<T> = std::result::Result<T, Refusal>;
pub const ERROR_MODEL_METRIC: &str = "plant.flowsheet.unmeasured_error_model/1";
pub const OUTPUT_METRIC: &str = "plant.flowsheet.inferred_flows/1";

/// No source ClaimId is attached to an unobserved amount. The stream itself is
/// bound to the complete topology and the explicit error-model declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnmeasuredFlowKey {
    pub stream: StreamId,
    pub channel: MaterialChannel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnmeasuredFlowModel {
    pub scope: FlowsheetScope,
    pub declaration_id: ClaimId,
    pub observations: Vec<ObservationKey>,
    pub unmeasured_streams: BTreeSet<StreamId>,
    /// Full covariance ONLY of the genuine observed coordinates, in kg².
    pub covariance: Vec<KilogramsSquared>,
    pub unbiased_gaussian_errors: bool,
    pub global_chi_square_limit: f64,
    pub normalized_adjustment_limit: Option<f64>,
    pub closure_tolerances_kg: BTreeMap<MaterialChannel, Kilograms>,
}
impl UnmeasuredFlowModel {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"UGEM", 1);
        enc.put_str(&self.scope.boundary);
        put_time(&mut enc, self.scope.interval);
        enc.put_str(&self.declaration_id.to_string());
        enc.put_u64(self.observations.len() as u64);
        for key in &self.observations {
            put_key(&mut enc, key);
        }
        enc.put_u64(self.unmeasured_streams.len() as u64);
        for stream in &self.unmeasured_streams {
            enc.put_str(stream.as_str());
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

/// Inferred quantities may be unique without any redundant measurement check.
/// `NoRedundancy` is not a passed gross-error test and supplies no statistic.
#[derive(Debug, Clone, PartialEq)]
pub enum GrossErrorDiagnostic {
    Available {
        chi_square: f64,
        degrees_of_freedom: usize,
    },
    NoRedundancy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitConservation {
    pub unit: UnitId,
    pub channel: MaterialChannel,
    pub has_unmeasured_terms: bool,
    /// Observed contribution alone; NOT an original full-balance residual when
    /// an unmeasured term is present.
    pub observed_contribution_exact_kg: ExactQty,
    pub observed_contribution_kg: Kilograms,
    pub reconciled_residual_kg: Kilograms,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferredBoundary {
    pub keys: Vec<BoundaryKey>,
    pub observed_contributions_exact_kg: Vec<ExactQty>,
    /// None wherever a role includes an unmeasured external stream or endpoint stock.
    pub observed_totals_kg: Vec<Option<Kilograms>>,
    pub estimates_kg: Vec<Kilograms>,
    pub posterior_covariance: Vec<KilogramsSquared>,
    /// One per channel in carrier/element order; no extra solver row is added.
    pub reconciled_residuals_kg: BTreeMap<MaterialChannel, Kilograms>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferredFlowsheet<K = UnmeasuredFlowKey> {
    pub scope: FlowsheetScope,
    pub observations: Vec<ObservationKey>,
    pub unmeasured: Vec<K>,
    pub observed_kg: Vec<Kilograms>,
    /// Canonical measured coordinates first, then canonical unmeasured keys.
    pub estimates_kg: Vec<Kilograms>,
    /// Only measured coordinates have measurement adjustments or diagnostics.
    pub measured_adjustments_kg: Vec<Kilograms>,
    pub posterior_covariance: Vec<KilogramsSquared>,
    pub measured_adjustment_covariance: Vec<KilogramsSquared>,
    pub normalized_adjustments: Vec<Option<f64>>,
    pub conservation: Vec<UnitConservation>,
    pub boundary: InferredBoundary,
    pub gross_error: GrossErrorDiagnostic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferredFlowsheetReceipt<M = UnmeasuredFlowModel, K = UnmeasuredFlowKey> {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<M>,
    claim: Claim<InferredFlowsheet<K>>,
    digest: String,
}
impl<M, K> InferredFlowsheetReceipt<M, K> {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<M> {
        &self.error_model
    }
    pub fn claim(&self) -> &Claim<InferredFlowsheet<K>> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RejectedInference<M = UnmeasuredFlowModel, K = UnmeasuredFlowKey> {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<M>,
    diagnostics: InferredFlowsheet<K>,
    refusal: Refusal,
    digest: String,
}
impl<M, K> RejectedInference<M, K> {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<M> {
        &self.error_model
    }
    pub fn diagnostics(&self) -> &InferredFlowsheet<K> {
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
pub enum UnmeasuredFlowOutcome<M = UnmeasuredFlowModel, K = UnmeasuredFlowKey> {
    Admitted(Box<InferredFlowsheetReceipt<M, K>>),
    Rejected(Box<RejectedInference<M, K>>),
}
impl<M, K> UnmeasuredFlowOutcome<M, K> {
    pub fn diagnostics(&self) -> &InferredFlowsheet<K> {
        match self {
            Self::Admitted(r) => &r.claim.value,
            Self::Rejected(r) => &r.diagnostics,
        }
    }
    pub fn admitted_claim(&self) -> Result<&Claim<InferredFlowsheet<K>>> {
        match self {
            Self::Admitted(r) => Ok(&r.claim),
            Self::Rejected(r) => Err(r.refusal.clone()),
        }
    }
}

/// Canonical order of genuine observed coordinates. Missing streams are allowed
/// only when explicitly named here and in the subsequently admitted model.
pub fn observation_order(
    inputs: &[Claim<FlowsheetInput>],
    unmeasured: &BTreeSet<StreamId>,
) -> Result<Vec<ObservationKey>> {
    Ok(coordinates(&admit_flowsheet_with_unmeasured(inputs, unmeasured)?)?.keys)
}

struct LatentGroup {
    location: ObservationLocation,
    boundary_role: Option<BoundaryRole>,
}
struct LatentLayout {
    keys: Vec<UnmeasuredQuantityKey>,
    groups: Vec<LatentGroup>,
    matrix: Vec<f64>,
}
fn latent_layout(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    unknown: &BTreeSet<ObservationLocation>,
) -> Result<LatentLayout> {
    if unknown.is_empty() {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare at least one unmeasured stream; use the fully measured owner otherwise",
        ));
    }
    let channel_count = data.channels.len();
    let p = unknown
        .len()
        .checked_mul(channel_count)
        .ok_or_else(|| numerical("unmeasured coordinate count overflow"))?;
    let m = graph.per_unit.len() * channel_count;
    if p > m {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "more unmeasured quantities than conservation rows; no unique flow inference",
        ));
    }
    if data.keys.len() + p > MAX_OBSERVATIONS {
        return Err(numerical(
            "unmeasured coordinates exceed the bounded numerical work limit",
        ));
    }
    let FlowsheetValue::Declaration { streams, .. } = &graph.declaration.value.value else {
        return Err(numerical("missing admitted topology"));
    };
    let routes: BTreeMap<_, _> = streams.iter().map(|s| (&s.id, &s.route)).collect();
    let units: BTreeMap<_, _> = graph
        .per_unit
        .keys()
        .enumerate()
        .map(|(i, u)| (u, i))
        .collect();
    let mut matrix = vec![0.0; m * p];
    let mut keys = Vec::with_capacity(p);
    let mut groups = Vec::with_capacity(unknown.len());
    for (j, location) in unknown.iter().enumerate() {
        let (endpoints, boundary_role) = match location {
            ObservationLocation::Transfer(stream) => {
                match routes
                    .get(stream)
                    .ok_or_else(|| numerical("missing admitted stream"))?
                {
                    StreamRoute::Feed { to } => (vec![(to, 1.0)], Some(BoundaryRole::Feed)),
                    StreamRoute::Internal { from, to, .. } => (vec![(from, -1.0), (to, 1.0)], None),
                    StreamRoute::Product { from } => {
                        (vec![(from, -1.0)], Some(BoundaryRole::Product))
                    }
                    StreamRoute::OtherOutflow { from } => {
                        (vec![(from, -1.0)], Some(BoundaryRole::OtherOutflow))
                    }
                }
            }
            ObservationLocation::Stocktake { unit, kind } => match kind {
                StocktakeKind::Opening => (vec![(unit, 1.0)], Some(BoundaryRole::OpeningInventory)),
                StocktakeKind::Closing => {
                    (vec![(unit, -1.0)], Some(BoundaryRole::ClosingInventory))
                }
            },
        };
        for (c, channel) in data.channels.iter().enumerate() {
            keys.push(UnmeasuredQuantityKey {
                location: location.clone(),
                channel: channel.clone(),
            });
            for (unit, sign) in &endpoints {
                let row = units
                    .get(unit)
                    .ok_or_else(|| numerical("missing admitted unit"))?
                    * channel_count
                    + c;
                matrix[row * p + j * channel_count + c] = *sign;
            }
        }
        groups.push(LatentGroup {
            location: location.clone(),
            boundary_role,
        });
    }
    Ok(LatentLayout {
        keys,
        groups,
        matrix,
    })
}

fn latent_physical_rejection(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    layout: &LatentLayout,
    estimates: &[f64],
) -> Result<Option<Refusal>> {
    let n = data.keys.len();
    if let Some(refusal) = physical_rejection(graph, data, &estimates[..n])? {
        return Ok(Some(refusal));
    }
    for (j, group) in layout.groups.iter().enumerate() {
        let start = n + j * data.channels.len();
        let values = &estimates[start..start + data.channels.len()];
        let (label, composition_label) = match &group.location {
            ObservationLocation::Transfer(stream) => (
                format!("stream {}", stream.as_str()),
                stream.as_str().to_owned(),
            ),
            ObservationLocation::Stocktake { unit, kind } => {
                let label = format!("{:?} stocktake at {}", kind, unit.as_str());
                (label.clone(), label)
            }
        };
        if values.iter().any(|v| *v < 0.0) {
            return Ok(Some(fail(
                RefusalCode::OutsidePhysicalDomain109,
                format!("inferred {label} has negative mass; no clipping"),
            )));
        }
        let mut amounts = MaterialAmounts {
            dry_solids_kg: represented_rational(values[0])?,
            water_kg: represented_rational(values[1])?,
            elements_kg: BTreeMap::new(),
        };
        for (c, channel) in data.channels.iter().enumerate().skip(2) {
            if let MaterialChannel::Element(element) = channel {
                amounts
                    .elements_kg
                    .insert(element.clone(), represented_rational(values[c])?);
            }
        }
        if let Err(mut refusal) = validate_amounts(&amounts, &graph.schema) {
            refusal.what = format!(
                "inferred composition at {}: {}",
                composition_label, refusal.what
            );
            return Ok(Some(refusal));
        }
    }
    Ok(None)
}

fn boundary_projection(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    layout: &LatentLayout,
    estimates: &[f64],
    covariance: &[f64],
    tolerances: &BTreeMap<MaterialChannel, Kilograms>,
) -> Result<InferredBoundary> {
    let n = data.keys.len();
    let k = data.channels.len();
    let mut keys = Vec::new();
    let mut groups = Vec::new();
    let mut original = Vec::new();
    let mut observed = Vec::new();
    for (role, sources) in role_rows(&graph.outer) {
        for (c, channel) in data.channels.iter().enumerate() {
            keys.push(BoundaryKey {
                role,
                channel: channel.clone(),
            });
            let group = sources
                .iter()
                .map(|s| offset(data, s.claim_id).map(|i| i + c))
                .collect::<Result<Vec<_>>>()?;
            let mut exact = ExactQty::from_integer(0);
            for &i in &group {
                exact = exact
                    .checked_add(data.exact[i])
                    .map_err(|_| numerical("observed boundary contribution overflow"))?;
            }
            original.push(exact);
            observed.push(true);
            groups.push(group);
        }
    }
    for (j, group) in layout.groups.iter().enumerate() {
        if let Some(role) = group.boundary_role {
            let role = role as usize;
            for c in 0..k {
                groups[role * k + c].push(n + j * k + c);
                observed[role * k + c] = false;
            }
        }
    }
    let totals = groups
        .iter()
        .map(|g| sum(g.iter().map(|&i| estimates[i])).map(Kilograms::new))
        .collect::<Result<Vec<_>>>()?;
    let mut observed_totals_kg = Vec::new();
    for (r, group) in groups.iter().enumerate() {
        let mut exact_fitted = ExactQty::from_integer(0);
        for &i in group {
            exact_fitted = exact_fitted
                .checked_add(represented_rational(estimates[i])?)
                .map_err(|_| numerical("represented inferred total overflow"))?;
        }
        let budget = tolerances[&keys[r].channel].value() / 5.0;
        let fitted_loss = exact_fitted
            .checked_sub(represented_rational(totals[r].value())?)
            .map_err(|_| numerical("inferred boundary total comparison overflow"))?
            .to_f64()
            .abs();
        if fitted_loss > budget {
            return Err(numerical(
                "inferred boundary role total loses representable mass",
            ));
        }
        if observed[r] {
            let total = original[r].to_f64();
            let loss = original[r]
                .checked_sub(represented_rational(total)?)
                .map_err(|_| numerical("observed boundary total comparison overflow"))?
                .to_f64()
                .abs();
            if loss > budget {
                return Err(numerical(
                    "observed boundary role total loses original exact mass",
                ));
            }
            observed_totals_kg.push(Some(Kilograms::new(total)));
        } else {
            observed_totals_kg.push(None);
        }
    }
    let posterior_covariance = project_group_covariance(covariance, estimates.len(), &groups)?;
    let mut reconciled_residuals_kg = BTreeMap::new();
    for (c, channel) in data.channels.iter().enumerate() {
        let residual = sum((0..5).map(|role| {
            (if positive(keys[role * k + c].role) {
                1.0
            } else {
                -1.0
            }) * totals[role * k + c].value()
        }))?;
        reconciled_residuals_kg.insert(channel.clone(), Kilograms::new(residual));
    }
    Ok(InferredBoundary {
        keys,
        observed_contributions_exact_kg: original,
        observed_totals_kg,
        estimates_kg: totals,
        posterior_covariance,
        reconciled_residuals_kg,
    })
}

/// Infer only identifiable missing stream quantities. Ambiguous unmeasured
/// cycles or parallel unknown outlets refuse before an inferred result exists.
/// A zero-redundancy inference may be physically admissible, but cannot test
/// the measurements for gross error; that distinction is explicit in the result.
pub fn reconcile_unmeasured_flowsheet(
    inputs: &[Claim<FlowsheetInput>],
    error_model: &Claim<UnmeasuredFlowModel>,
) -> Result<UnmeasuredFlowOutcome> {
    let model = &error_model.value;
    inventory::admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let unknown = model
        .unmeasured_streams
        .iter()
        .cloned()
        .map(ObservationLocation::Transfer)
        .collect();
    reconcile_selected(
        inputs,
        ModelView {
            source: error_model,
            scope: &model.scope,
            declaration_id: model.declaration_id,
            observations: &model.observations,
            covariance: &model.covariance,
            unbiased_gaussian_errors: model.unbiased_gaussian_errors,
            global_chi_square_limit: model.global_chi_square_limit,
            normalized_adjustment_limit: model.normalized_adjustment_limit,
            closure_tolerances_kg: &model.closure_tolerances_kg,
        },
        &unknown,
        InferenceEncoding {
            frame: *b"UGRR",
            metric: OUTPUT_METRIC,
            model_name: "unmeasured-flow",
            diagnostic_name: "partial-flow",
            key: |key| match key.location {
                ObservationLocation::Transfer(stream) => Ok(UnmeasuredFlowKey {
                    stream,
                    channel: key.channel,
                }),
                ObservationLocation::Stocktake { .. } => {
                    Err(numerical("stocktake in flow-only inference"))
                }
            },
            put_key: |enc, key| {
                enc.put_str(key.stream.as_str());
                put_channel(enc, &key.channel);
            },
        },
    )
}

/// Borrowed numerical policy, independent of each public source-model schema.
/// The wrappers admit the original model Claim before constructing this view.
struct ModelView<'a, M> {
    source: &'a Claim<M>,
    scope: &'a FlowsheetScope,
    declaration_id: ClaimId,
    observations: &'a [ObservationKey],
    covariance: &'a [KilogramsSquared],
    unbiased_gaussian_errors: bool,
    global_chi_square_limit: f64,
    normalized_adjustment_limit: Option<f64>,
    closure_tolerances_kg: &'a BTreeMap<MaterialChannel, Kilograms>,
}

/// The flow-only wire format remains unchanged; generalized quantities have
/// their own metric/frame and bind typed endpoint identities in every key.
struct InferenceEncoding<K> {
    frame: [u8; 4],
    metric: &'static str,
    model_name: &'static str,
    diagnostic_name: &'static str,
    key: fn(UnmeasuredQuantityKey) -> Result<K>,
    put_key: fn(&mut Encoder, &K),
}

fn reconcile_selected<M: Clone, K>(
    inputs: &[Claim<FlowsheetInput>],
    model: ModelView<'_, M>,
    unknown: &BTreeSet<ObservationLocation>,
    encoding: InferenceEncoding<K>,
) -> Result<UnmeasuredFlowOutcome<M, K>> {
    let error_model = model.source;
    let graph = admit_flowsheet_with_unmeasured_quantities(inputs, unknown)?;
    let data = coordinates(&graph)?;
    let layout = latent_layout(&graph, &data, unknown)?;
    let n = data.keys.len();
    let p = layout.keys.len();
    if model.scope != &graph.declaration.value.scope
        || model.declaration_id != graph.declaration.claim_id
        || model.observations != data.keys.as_slice()
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            format!(
                "{} model must bind the exact topology, scope and genuine observations",
                encoding.model_name
            ),
        ));
    }
    if model.covariance.len() != n * n
        || !model.closure_tolerances_kg.keys().eq(data.channels.iter())
    {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "covariance must cover only measured coordinates, with every channel tolerance explicit",
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
            "declare unbiased Gaussian errors, finite positive policies and nonnegative channel allowances",
        ));
    }
    let mut observed = Vec::with_capacity(n);
    let mut budgets = Vec::with_capacity(n);
    for (i, q) in data.exact.iter().enumerate() {
        let (value, budget) = nominal_with_precision_budget(
            *q,
            model.covariance[i * n + i],
            model.closure_tolerances_kg[&data.keys[i].channel],
            data.offsets.len(),
        )?;
        observed.push(value);
        budgets.push(budget);
    }
    let (measured_matrix, original_rows) = constraints(&graph, &data)?;
    let covariance: Vec<_> = model.covariance.iter().map(|v| v.value()).collect();
    let result = reconcile_with_unmeasured(
        &observed,
        &covariance,
        &measured_matrix,
        &layout.matrix,
        &vec![0.0; original_rows.len()],
        p,
    )?;
    let mut conservation = Vec::with_capacity(original_rows.len());
    for (r, original) in original_rows.into_iter().enumerate() {
        if (result.raw_observed_residuals[r] - original.raw_residual_exact_kg.to_f64()).abs()
            > model.closure_tolerances_kg[&original.channel].value()
        {
            return Err(numerical(
                "original exact measured contribution is lost during inference",
            ));
        }
        conservation.push(UnitConservation {
            unit: original.unit,
            channel: original.channel,
            has_unmeasured_terms: layout.matrix[r * p..(r + 1) * p].iter().any(|a| *a != 0.0),
            observed_contribution_exact_kg: original.raw_residual_exact_kg,
            observed_contribution_kg: Kilograms::new(result.raw_observed_residuals[r]),
            reconciled_residual_kg: Kilograms::new(result.reconciled_residuals[r]),
        });
    }
    for i in 0..n {
        if ((result.measured_estimates[i] - observed[i]) - result.measured_adjustments[i]).abs()
            > budgets[i]
        {
            return Err(numerical(
                "measured fitted value cannot represent its adjustment",
            ));
        }
    }
    let fitted: Vec<_> = result
        .measured_estimates
        .iter()
        .chain(&result.latent_estimates)
        .copied()
        .collect();
    let mut rejection = latent_physical_rejection(&graph, &data, &layout, &fitted)?;
    if rejection.is_none()
        && conservation.iter().any(|row| {
            row.reconciled_residual_kg.value().abs()
                > model.closure_tolerances_kg[&row.channel].value()
        })
    {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            "inferred unit balance exceeds the declared channel allowance",
        ));
    }
    let gross_error = if result.degrees_of_freedom == 0 {
        GrossErrorDiagnostic::NoRedundancy
    } else {
        if rejection.is_none() && result.chi_square > model.global_chi_square_limit {
            rejection = Some(fail(
                RefusalCode::Conservation101,
                format!(
                    "{} global chi-square {} exceeds declared limit {}; all observations retained",
                    encoding.diagnostic_name, result.chi_square, model.global_chi_square_limit
                ),
            ));
        }
        if rejection.is_none()
            && let Some(limit) = model.normalized_adjustment_limit
            && result
                .normalized_adjustments
                .iter()
                .flatten()
                .any(|v| v.abs() > limit)
        {
            rejection = Some(fail(
                RefusalCode::Conservation101,
                "measured normalized adjustment exceeds the declared simultaneous policy",
            ));
        }
        GrossErrorDiagnostic::Available {
            chi_square: result.chi_square,
            degrees_of_freedom: result.degrees_of_freedom,
        }
    };
    let boundary = boundary_projection(
        &graph,
        &data,
        &layout,
        &fitted,
        &result.posterior_covariance,
        model.closure_tolerances_kg,
    )?;
    if rejection.is_none()
        && boundary
            .reconciled_residuals_kg
            .iter()
            .any(|(channel, value)| {
                value.value().abs() > model.closure_tolerances_kg[channel].value()
            })
    {
        rejection = Some(fail(
            RefusalCode::Conservation101,
            "inferred external boundary exceeds its declared channel allowance",
        ));
    }
    let mut meta = Metadata::new();
    let mut enc = Encoder::new_frame(encoding.frame, 1);
    enc.put_u64(graph.inputs.len() as u64);
    for source in &graph.inputs {
        inventory::put_source_envelope(&mut enc, *source, source.value.value.key().0);
        meta.include(*source)?;
    }
    inventory::put_source_envelope(&mut enc, error_model, 3);
    meta.include(error_model)?;
    let diagnostics = InferredFlowsheet {
        scope: model.scope.clone(),
        observations: data.keys,
        unmeasured: layout
            .keys
            .into_iter()
            .map(encoding.key)
            .collect::<Result<Vec<_>>>()?,
        observed_kg: observed.into_iter().map(Kilograms::new).collect(),
        estimates_kg: fitted.into_iter().map(Kilograms::new).collect(),
        measured_adjustments_kg: result
            .measured_adjustments
            .into_iter()
            .map(Kilograms::new)
            .collect(),
        posterior_covariance: result
            .posterior_covariance
            .into_iter()
            .map(KilogramsSquared::new)
            .collect(),
        measured_adjustment_covariance: result
            .adjustment_covariance
            .into_iter()
            .map(KilogramsSquared::new)
            .collect(),
        normalized_adjustments: result.normalized_adjustments,
        conservation,
        boundary,
        gross_error,
    };
    put_diagnostics(&mut enc, &diagnostics, encoding.put_key);
    enc.put_u8(u8::from(rejection.is_some()));
    if let Some(refusal) = &rejection {
        enc.put_str(refusal.code);
        enc.put_str(&refusal.what);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let inputs = graph.inputs.into_iter().cloned().collect();
    if let Some(refusal) = rejection {
        return Ok(UnmeasuredFlowOutcome::Rejected(Box::new(
            RejectedInference {
                inputs,
                error_model: error_model.clone(),
                diagnostics,
                refusal,
                digest,
            },
        )));
    }
    let claim = meta.into_claim(encoding.metric, diagnostics, &digest);
    Ok(UnmeasuredFlowOutcome::Admitted(Box::new(
        InferredFlowsheetReceipt {
            inputs,
            error_model: error_model.clone(),
            claim,
            digest,
        },
    )))
}

fn put_diagnostics<K>(
    enc: &mut Encoder,
    d: &InferredFlowsheet<K>,
    put_unmeasured: fn(&mut Encoder, &K),
) {
    enc.put_u64(d.observations.len() as u64);
    for key in &d.observations {
        put_key(enc, key);
    }
    enc.put_u64(d.unmeasured.len() as u64);
    for key in &d.unmeasured {
        put_unmeasured(enc, key);
    }
    for values in [&d.observed_kg, &d.estimates_kg, &d.measured_adjustments_kg] {
        for value in values {
            enc.put_u64(value.value().to_bits());
        }
    }
    for values in [&d.posterior_covariance, &d.measured_adjustment_covariance] {
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
        enc.put_u8(u8::from(row.has_unmeasured_terms));
        put_qty(enc, row.observed_contribution_exact_kg);
        enc.put_u64(row.observed_contribution_kg.value().to_bits());
        enc.put_u64(row.reconciled_residual_kg.value().to_bits());
    }
    for (i, key) in d.boundary.keys.iter().enumerate() {
        enc.put_u8(key.role as u8);
        put_channel(enc, &key.channel);
        put_qty(enc, d.boundary.observed_contributions_exact_kg[i]);
        put_optional(enc, d.boundary.observed_totals_kg[i].map(|v| v.value()));
        enc.put_u64(d.boundary.estimates_kg[i].value().to_bits());
    }
    for value in &d.boundary.posterior_covariance {
        enc.put_u64(value.value().to_bits());
    }
    for (channel, value) in &d.boundary.reconciled_residuals_kg {
        put_channel(enc, channel);
        enc.put_u64(value.value().to_bits());
    }
    match d.gross_error {
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
