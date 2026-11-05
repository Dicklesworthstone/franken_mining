//! DESIGN: infer explicitly unmeasured extensive flows from observed stocks and
//! other flows (plan §§20.2–20.3 R1, BD-149/177). All channels of a named stream
//! are unmeasured; both endpoint stocktakes for EVERY unit remain observations.
//! The shared numerical owner solves measured/latent GLS and refuses deficient
//! observability. No observation, variance, prior, balancing plug or zero is
//! invented for a missing flow. This is not a temporal/bias R2 model.
//! Determinism: DeterministicPerIsa

use super::{
    BoundaryKey, Coordinates, MaterialChannel, ObservationKey, constraints, coordinates, fail,
    numerical, offset, physical_rejection, positive, project_group_covariance, put_channel,
    put_key, put_optional, role_rows, sum,
};
use crate::flowsheet::{
    AdmittedFlowsheet, FlowsheetInput, FlowsheetScope, FlowsheetValue, MaterialAmounts, StreamId,
    StreamRoute, UnitId, admit_flowsheet_with_unmeasured, put_qty, put_time, validate_amounts,
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

/// Inferred flows may be unique without any redundant measurement check.
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
    /// None wherever a role includes an unmeasured external stream.
    pub observed_totals_kg: Vec<Option<Kilograms>>,
    pub estimates_kg: Vec<Kilograms>,
    pub posterior_covariance: Vec<KilogramsSquared>,
    /// One per channel in carrier/element order; no extra solver row is added.
    pub reconciled_residuals_kg: BTreeMap<MaterialChannel, Kilograms>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InferredFlowsheet {
    pub scope: FlowsheetScope,
    pub observations: Vec<ObservationKey>,
    pub unmeasured: Vec<UnmeasuredFlowKey>,
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
pub struct InferredFlowsheetReceipt {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<UnmeasuredFlowModel>,
    claim: Claim<InferredFlowsheet>,
    digest: String,
}
impl InferredFlowsheetReceipt {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<UnmeasuredFlowModel> {
        &self.error_model
    }
    pub fn claim(&self) -> &Claim<InferredFlowsheet> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RejectedInference {
    inputs: Vec<Claim<FlowsheetInput>>,
    error_model: Claim<UnmeasuredFlowModel>,
    diagnostics: InferredFlowsheet,
    refusal: Refusal,
    digest: String,
}
impl RejectedInference {
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<UnmeasuredFlowModel> {
        &self.error_model
    }
    pub fn diagnostics(&self) -> &InferredFlowsheet {
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
pub enum UnmeasuredFlowOutcome {
    Admitted(Box<InferredFlowsheetReceipt>),
    Rejected(Box<RejectedInference>),
}
impl UnmeasuredFlowOutcome {
    pub fn diagnostics(&self) -> &InferredFlowsheet {
        match self {
            Self::Admitted(r) => &r.claim.value,
            Self::Rejected(r) => &r.diagnostics,
        }
    }
    pub fn admitted_claim(&self) -> Result<&Claim<InferredFlowsheet>> {
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

struct LatentLayout {
    keys: Vec<UnmeasuredFlowKey>,
    routes: BTreeMap<StreamId, StreamRoute>,
    matrix: Vec<f64>,
}
fn latent_layout(
    graph: &AdmittedFlowsheet<'_>,
    data: &Coordinates,
    unknown: &BTreeSet<StreamId>,
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
    let routes: BTreeMap<_, _> = streams
        .iter()
        .filter(|s| unknown.contains(&s.id))
        .map(|s| (s.id.clone(), s.route.clone()))
        .collect();
    let units: BTreeMap<_, _> = graph
        .per_unit
        .keys()
        .enumerate()
        .map(|(i, u)| (u, i))
        .collect();
    let mut matrix = vec![0.0; m * p];
    let mut keys = Vec::with_capacity(p);
    for (j, (stream, route)) in routes.iter().enumerate() {
        let endpoints = match route {
            StreamRoute::Feed { to } => vec![(to, 1.0)],
            StreamRoute::Internal { from, to, .. } => vec![(from, -1.0), (to, 1.0)],
            StreamRoute::Product { from } | StreamRoute::OtherOutflow { from } => {
                vec![(from, -1.0)]
            }
        };
        for (c, channel) in data.channels.iter().enumerate() {
            keys.push(UnmeasuredFlowKey {
                stream: stream.clone(),
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
    }
    Ok(LatentLayout {
        keys,
        routes,
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
    for (j, stream) in layout.routes.keys().enumerate() {
        let start = n + j * data.channels.len();
        let values = &estimates[start..start + data.channels.len()];
        if values.iter().any(|v| *v < 0.0) {
            return Ok(Some(fail(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "inferred stream {} has negative mass; no clipping",
                    stream.as_str()
                ),
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
                stream.as_str(),
                refusal.what
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
    for (j, route) in layout.routes.values().enumerate() {
        let role = match route {
            StreamRoute::Feed { .. } => Some(1),
            StreamRoute::Product { .. } => Some(2),
            StreamRoute::OtherOutflow { .. } => Some(3),
            StreamRoute::Internal { .. } => None,
        };
        if let Some(role) = role {
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
    let graph = admit_flowsheet_with_unmeasured(inputs, &model.unmeasured_streams)?;
    let data = coordinates(&graph)?;
    let layout = latent_layout(&graph, &data, &model.unmeasured_streams)?;
    let n = data.keys.len();
    let p = layout.keys.len();
    if model.scope != graph.declaration.value.scope
        || model.declaration_id != graph.declaration.claim_id
        || model.observations != data.keys
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "unmeasured-flow model must bind the exact topology, scope and genuine observations",
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
                    "partial-flow global chi-square {} exceeds declared limit {}; all observations retained",
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
        &model.closure_tolerances_kg,
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
    let mut enc = Encoder::new_frame(*b"UGRR", 1);
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
        unmeasured: layout.keys,
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
    put_diagnostics(&mut enc, &diagnostics);
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
    let claim = meta.into_claim(OUTPUT_METRIC, diagnostics, &digest);
    Ok(UnmeasuredFlowOutcome::Admitted(Box::new(
        InferredFlowsheetReceipt {
            inputs,
            error_model: error_model.clone(),
            claim,
            digest,
        },
    )))
}

fn put_diagnostics(enc: &mut Encoder, d: &InferredFlowsheet) {
    enc.put_u64(d.observations.len() as u64);
    for key in &d.observations {
        put_key(enc, key);
    }
    enc.put_u64(d.unmeasured.len() as u64);
    for key in &d.unmeasured {
        enc.put_str(key.stream.as_str());
        put_channel(enc, &key.channel);
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
