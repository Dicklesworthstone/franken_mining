//! DESIGN: single-window partial GLS for explicitly unmeasured transfers and
//! endpoint inventories (plan §§20.2–20.3 R1, BD-154/177). Each latent location
//! contains every declared extensive channel, without a synthetic observation
//! or prior. Both endpoints of one unit are not individually identifiable.
//! Determinism: DeterministicPerIsa

use super::{
    AdditivityClass, BTreeMap, BTreeSet, Claim, ClaimId, Encoder, FlowsheetInput, FlowsheetScope,
    InferenceEncoding, InferredFlowsheet, InferredFlowsheetReceipt, Kilograms, KilogramsSquared,
    MaterialChannel, ModelView, ObservationKey, ObservationLocation, RefusalCode,
    RejectedInference, Result, StocktakeKind, UnmeasuredFlowOutcome,
    admit_flowsheet_with_unmeasured_quantities, blake3_hash, coordinates, fail, inventory,
    put_channel, put_key, put_optional, put_time, reconcile_selected,
};

pub const ERROR_MODEL_METRIC: &str = "plant.flowsheet.unmeasured_quantity_error_model/1";
pub const OUTPUT_METRIC: &str = "plant.flowsheet.inferred_quantities/1";

/// An inferred location/channel has no measurement ClaimId. Its identity is
/// bound to the exact topology and the explicit source error-model Claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnmeasuredQuantityKey {
    pub location: ObservationLocation,
    pub channel: MaterialChannel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnmeasuredQuantityModel {
    pub scope: FlowsheetScope,
    pub declaration_id: ClaimId,
    pub observations: Vec<ObservationKey>,
    /// Every selected location is absent from the measured source Claims.
    /// At most one endpoint per unit can be inferred in this single window.
    pub unmeasured: BTreeSet<ObservationLocation>,
    /// Full covariance ONLY of genuine observations, in their canonical order.
    pub covariance: Vec<KilogramsSquared>,
    pub unbiased_gaussian_errors: bool,
    pub global_chi_square_limit: f64,
    pub normalized_adjustment_limit: Option<f64>,
    pub closure_tolerances_kg: BTreeMap<MaterialChannel, Kilograms>,
}

impl UnmeasuredQuantityModel {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"UQEM", 1);
        enc.put_str(&self.scope.boundary);
        put_time(&mut enc, self.scope.interval);
        enc.put_str(&self.declaration_id.to_string());
        enc.put_u64(self.observations.len() as u64);
        for key in &self.observations {
            put_key(&mut enc, key);
        }
        enc.put_u64(self.unmeasured.len() as u64);
        for location in &self.unmeasured {
            put_location(&mut enc, location);
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

pub type InferredQuantities = InferredFlowsheet<UnmeasuredQuantityKey>;
pub type InferredQuantitiesReceipt =
    InferredFlowsheetReceipt<UnmeasuredQuantityModel, UnmeasuredQuantityKey>;
pub type RejectedQuantityInference =
    RejectedInference<UnmeasuredQuantityModel, UnmeasuredQuantityKey>;
pub type UnmeasuredQuantityOutcome =
    UnmeasuredFlowOutcome<UnmeasuredQuantityModel, UnmeasuredQuantityKey>;

/// Canonical order of the genuine measurements. An omitted endpoint is allowed
/// only when explicitly selected; an undeclared omission is never a zero.
pub fn observation_order_for_quantities(
    inputs: &[Claim<FlowsheetInput>],
    unmeasured: &BTreeSet<ObservationLocation>,
) -> Result<Vec<ObservationKey>> {
    Ok(coordinates(&admit_flowsheet_with_unmeasured_quantities(
        inputs, unmeasured,
    )?)?
    .keys)
}

/// Infer identifiable stream and endpoint quantities jointly with measured
/// adjustments and their full joint covariance. Rank failure refuses before a
/// result; physical or declared acceptance failures retain rejected diagnostics
/// and never produce an admitted result Claim. This is not temporal smoothing.
pub fn reconcile_unmeasured_quantities(
    inputs: &[Claim<FlowsheetInput>],
    error_model: &Claim<UnmeasuredQuantityModel>,
) -> Result<UnmeasuredQuantityOutcome> {
    let model = &error_model.value;
    inventory::admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if model.unmeasured.is_empty() {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "declare at least one unmeasured quantity; use the fully measured owner otherwise",
        ));
    }
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
        &model.unmeasured,
        InferenceEncoding {
            frame: *b"UQRR",
            metric: OUTPUT_METRIC,
            model_name: "unmeasured-quantity",
            diagnostic_name: "partial-quantity",
            key: Ok,
            put_key: |enc, key| {
                put_location(enc, &key.location);
                put_channel(enc, &key.channel);
            },
        },
    )
}

fn put_location(enc: &mut Encoder, location: &ObservationLocation) {
    match location {
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
}

#[cfg(test)]
mod tests;
