//! DESIGN: reconcile the five measured elemental inventory totals using a
//! source-declared full covariance and gross-error policy (plan §§17.5, 20.3).
//! Original observations remain unchanged. A successful nominal calculation
//! does not audit completeness, infer sensor bias or establish P754 compliance.
//! The shared numerical owner supplies WLS estimates and their full covariance.
//! This does not manufacture an ExactQty balance from rounded fitted values.
//! Determinism: DeterministicPerIsa

use super::inventory::{self, BalanceScope, BalanceValue, MetalBalanceInput};
use fmine_claim::{
    AdditivityClass, Claim, ClaimId, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::reconciliation::reconcile_linear;
use fmine_units::{ExactQty, Mass, Qty};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, Refusal>;
pub const ERROR_MODEL_METRIC: &str = "plant.inventory.error_model/1";
pub const OUTPUT_METRIC: &str = "plant.inventory.reconciled_interval/1";
const BALANCE_ROW: [f64; 5] = [1.0, 1.0, -1.0, -1.0, -1.0];

/// Squared-mass marker; every covariance entry uses kg², including signed cross terms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquaredMass;
/// Every mass coordinate in this module is total elemental kg, across all phases.
pub type Kilograms = Qty<Mass>;
pub type KilogramsSquared = Qty<SquaredMass>;

/// Original measurement-error declaration bound to the five specific source
/// claim identities. Order: opening, feed, product, other outflow, closing.
/// This structured model has a deterministic envelope; the covariance inside
/// it describes uncertainty in the observations, not uncertainty in these
/// explicitly supplied model parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryErrorModel {
    pub scope: BalanceScope,
    pub observation_ids: [ClaimId; 5],
    /// Full symmetric positive-definite covariance. No independence is inferred.
    pub covariance: [KilogramsSquared; 25],
    /// Explicit model assumption required for the chi-square interpretation.
    pub unbiased_gaussian_errors: bool,
    /// Caller-declared rejection threshold, with its source in this model's anchors.
    pub global_chi_square_limit: f64,
    /// Optional declared simultaneous-test policy. Diagnostics are correlated;
    /// this threshold is not derived from an uncorrected individual p-value.
    pub normalized_adjustment_limit: Option<f64>,
    /// Required domain closure allowance in kg, independent of numerical audits.
    pub closure_tolerance_kg: Kilograms,
}
impl InventoryErrorModel {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PIME", 1);
        enc.put_str(&self.scope.boundary);
        enc.put_i64(self.scope.interval.start_ns);
        enc.put_i64(self.scope.interval.end_ns);
        enc.put_str(&self.scope.element);
        enc.put_str(&self.scope.measurand);
        enc.put_str(inventory::MASS_BASIS);
        for id in self.observation_ids {
            enc.put_str(&id.to_string());
        }
        for v in self.covariance {
            enc.put_u64(v.value().to_bits());
        }
        enc.put_u8(u8::from(self.unbiased_gaussian_errors));
        enc.put_u64(self.global_chi_square_limit.to_bits());
        enc.put_u8(u8::from(self.normalized_adjustment_limit.is_some()));
        if let Some(v) = self.normalized_adjustment_limit {
            enc.put_u64(v.to_bits());
        }
        enc.put_u64(self.closure_tolerance_kg.value().to_bits());
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Conditional Gaussian result, in the same five-role order as the error model.
/// Full covariance is essential: the five reconciled values are not independent.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryReconciliation {
    pub scope: BalanceScope,
    pub observed_kg: [Kilograms; 5],
    pub estimates_kg: [Kilograms; 5],
    pub adjustments_kg: [Kilograms; 5],
    pub posterior_covariance: [KilogramsSquared; 25],
    pub normalized_adjustments: [Option<f64>; 5],
    pub raw_residual_kg: Kilograms,
    /// Original exact nominal imbalance, before any binary64 conversion.
    pub raw_residual_exact_kg: ExactQty,
    pub reconciled_residual_kg: Kilograms,
    pub chi_square: f64,
    pub degrees_of_freedom: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReconciledInventoryReceipt {
    inputs: Vec<Claim<MetalBalanceInput>>,
    error_model: Claim<InventoryErrorModel>,
    claim: Claim<InventoryReconciliation>,
    digest: String,
}
impl ReconciledInventoryReceipt {
    pub fn inputs(&self) -> &[Claim<MetalBalanceInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<InventoryErrorModel> {
        &self.error_model
    }
    pub fn claim(&self) -> &Claim<InventoryReconciliation> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Gross-error/physical rejection retains the original evidence and numerical
/// diagnostics but carries no admitted balance Claim. No measurement is deleted.
#[derive(Debug, Clone, PartialEq)]
pub struct RejectedInventoryReconciliation {
    inputs: Vec<Claim<MetalBalanceInput>>,
    error_model: Claim<InventoryErrorModel>,
    diagnostics: InventoryReconciliation,
    refusal: Refusal,
    digest: String,
}
impl RejectedInventoryReconciliation {
    pub fn inputs(&self) -> &[Claim<MetalBalanceInput>] {
        &self.inputs
    }
    pub fn error_model(&self) -> &Claim<InventoryErrorModel> {
        &self.error_model
    }
    pub fn diagnostics(&self) -> &InventoryReconciliation {
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
pub enum InventoryReconciliationOutcome {
    Admitted(Box<ReconciledInventoryReceipt>),
    Rejected(Box<RejectedInventoryReconciliation>),
}
impl InventoryReconciliationOutcome {
    pub fn diagnostics(&self) -> &InventoryReconciliation {
        match self {
            Self::Admitted(r) => &r.claim.value,
            Self::Rejected(r) => &r.diagnostics,
        }
    }
    /// A rejected candidate cannot become an accounting Claim through this API.
    pub fn admitted_claim(&self) -> Result<&Claim<InventoryReconciliation>> {
        match self {
            Self::Admitted(r) => Ok(&r.claim),
            Self::Rejected(r) => Err(r.refusal.clone()),
        }
    }
}

fn fail(code: RefusalCode, message: impl Into<String>) -> Refusal {
    Refusal::new(
        code,
        format!("inventory reconciliation: {}", message.into()),
    )
}
fn role(value: BalanceValue) -> usize {
    match value {
        BalanceValue::Population { .. } => 0,
        BalanceValue::OpeningInventory(_) => 1,
        BalanceValue::Feed(_) => 2,
        BalanceValue::Product(_) => 3,
        BalanceValue::OtherOutflow(_) => 4,
        BalanceValue::ClosingInventory(_) => 5,
    }
}

// Compare the original rational against the exact binary64 value. A covariance
// smaller than source conversion error cannot support an honest WLS statistic.
pub(crate) fn represented_rational(value: f64) -> Result<ExactQty> {
    if value == 0.0 {
        return Ok(ExactQty::from_integer(0));
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = (bits & ((1_u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1_u64 << 52 };
    let mut shift = if exponent == 0 {
        -1074
    } else {
        exponent - 1023 - 52
    };
    while shift < 0 && mantissa.is_multiple_of(2) {
        mantissa /= 2;
        shift += 1;
    }
    // Rejected fitted candidates can be negative. Preserve their sign during
    // exact diagnostics; nominal physical admission remains nonnegative.
    let mantissa = if value.is_sign_negative() {
        -(mantissa as i128)
    } else {
        mantissa as i128
    };
    let (num, den) = if shift >= 0 {
        let num = mantissa
            .checked_mul(
                1_i128
                    .checked_shl(shift as u32)
                    .filter(|factor| *factor > 0)
                    .ok_or_else(|| {
                        fail(
                            RefusalCode::NumericalCertificateFailed207,
                            "binary64 mass is outside exact comparison range",
                        )
                    })?,
            )
            .ok_or_else(|| {
                fail(
                    RefusalCode::NumericalCertificateFailed207,
                    "binary64 mass is outside exact comparison range",
                )
            })?;
        (num, 1)
    } else {
        let den = 1_i128
            .checked_shl((-shift) as u32)
            .filter(|v| *v > 0)
            .ok_or_else(|| {
                fail(
                    RefusalCode::NumericalCertificateFailed207,
                    "binary64 denominator is outside exact comparison range",
                )
            })?;
        (mantissa, den)
    };
    ExactQty::new(num, den).map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "binary64 comparison overflow",
        )
    })
}

/// Admit an exact nominal mass under the shared conversion/adjustment budget.
/// Splitting the closure allowance among every coordinate bounds accumulated
/// conversion loss without treating numerical precision as measurement error.
pub(crate) fn nominal_with_precision_budget(
    q: ExactQty,
    variance: KilogramsSquared,
    closure_allowance: Kilograms,
    coordinates: usize,
) -> Result<(f64, f64)> {
    let observed = q.to_f64();
    if !observed.is_finite() || q.signum() < 0 || (q.signum() > 0 && observed == 0.0) {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "nominal kg value is unrepresentable",
        ));
    }
    let variance = variance.value();
    if !variance.is_finite() || variance <= 0.0 {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "every measurement variance must be finite and positive",
        ));
    }
    let allowance = (variance.sqrt() * 1e-6).min(closure_allowance.value() / coordinates as f64);
    let loss = q
        .checked_sub(represented_rational(observed)?)
        .map_err(|_| {
            fail(
                RefusalCode::NumericalCertificateFailed207,
                "exact conversion comparison overflow",
            )
        })?
        .to_f64()
        .abs();
    if loss > allowance {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "nominal conversion exceeds declared closure or one-millionth-sigma numerical budget",
        ));
    }
    Ok((observed, allowance))
}

pub(crate) struct Metadata {
    pub(crate) evidence: EvidenceClass,
    pub(crate) epistemic: EpistemicStatus,
    pub(crate) state: KnowledgeState,
    pub(crate) confidentiality: Confidentiality,
    pub(crate) uu: BTreeMap<String, UuTerm>,
    pub(crate) parents: BTreeSet<ClaimId>,
    pub(crate) anchors: BTreeSet<fmine_core::anchor::SourceAnchor>,
}
impl Metadata {
    pub(crate) fn new() -> Self {
        Self {
            evidence: EvidenceClass::Derived,
            epistemic: EpistemicStatus::KnownKnown,
            state: KnowledgeState::Known,
            confidentiality: Confidentiality::public(),
            uu: BTreeMap::new(),
            parents: BTreeSet::new(),
            anchors: BTreeSet::new(),
        }
    }
    pub(crate) fn include<T>(&mut self, input: &Claim<T>) -> Result<()> {
        self.evidence = self.evidence.meet(input.evidence);
        self.epistemic = self.epistemic.and(input.epistemic);
        self.state = self.state.and(input.state);
        self.confidentiality = self.confidentiality.join(&input.confidentiality);
        self.parents.insert(input.claim_id);
        self.anchors.extend(input.anchors.iter().cloned());
        for term in &input.uu_terms {
            if let Some(previous) = self.uu.insert(term.scenario_id.clone(), term.clone())
                && previous != *term
            {
                return Err(fail(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting unknown-unknown metadata",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn into_claim<T>(self, metric: &str, value: T, digest: &str) -> Claim<T> {
        Claim::new(
            metric,
            value,
            digest,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::NonAdditive,
            self.evidence,
            self.epistemic,
            self.state.and(KnowledgeState::Estimated),
            self.confidentiality,
            self.uu.into_values().collect(),
            self.parents.into_iter().collect(),
            self.anchors.into_iter().collect(),
        )
    }
}

/// Reconcile the same complete six-role input used by `account_interval`, with
/// an additional source-bound covariance and threshold declaration. Nominal
/// point observations are preserved, and arithmetic never upgrades their
/// evidence. All five measurements require positive uncertainty variance;
/// missing/latent measurements and singular correlated errors need a different
/// explicitly declared model. Negative fitted physical totals reject, never clamp.
pub fn reconcile_inventory(
    inputs: &[Claim<MetalBalanceInput>],
    error_model: &Claim<InventoryErrorModel>,
) -> Result<InventoryReconciliationOutcome> {
    if inputs.len() != 6 {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "six complete boundary roles are required",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by_key(|input| role(input.value.value));
    for (i, input) in inputs.iter().enumerate() {
        inventory::admit(input)?;
        if role(input.value.value) != i {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "duplicate or missing role",
            ));
        }
        if input.value.scope != inputs[0].value.scope {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "incompatible input scopes",
            ));
        }
    }
    if inputs[0].value.value != (BalanceValue::Population { complete: true }) {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "complete boundary population must be declared",
        ));
    }
    let model = &error_model.value;
    inventory::admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if model.scope != inputs[0].value.scope
        || model.observation_ids != core::array::from_fn(|i| inputs[i + 1].claim_id)
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "error model does not name these exact observations and scope",
        ));
    }
    if !model.unbiased_gaussian_errors
        || !model.global_chi_square_limit.is_finite()
        || model.global_chi_square_limit <= 0.0
        || model
            .normalized_adjustment_limit
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        || !model.closure_tolerance_kg.value().is_finite()
        || model.closure_tolerance_kg.value() < 0.0
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "declare finite positive test limits, unbiased Gaussian errors and a nonnegative kg closure allowance",
        ));
    }
    let mut observations = [0.0; 5];
    let mut precision_allowance = [0.0; 5];
    let mut raw_exact = ExactQty::from_integer(0);
    for (i, input) in inputs[1..].iter().enumerate() {
        let q = match input.value.value {
            BalanceValue::OpeningInventory(q)
            | BalanceValue::Feed(q)
            | BalanceValue::Product(q)
            | BalanceValue::OtherOutflow(q)
            | BalanceValue::ClosingInventory(q) => q,
            BalanceValue::Population { .. } => {
                return Err(fail(RefusalCode::BasisMismatch105, "mass role required"));
            }
        };
        (observations[i], precision_allowance[i]) = nominal_with_precision_budget(
            q,
            model.covariance[i * 5 + i],
            model.closure_tolerance_kg,
            5,
        )?;
        raw_exact = if BALANCE_ROW[i] > 0.0 {
            raw_exact.checked_add(q)
        } else {
            raw_exact.checked_sub(q)
        }
        .map_err(|_| {
            fail(
                RefusalCode::NumericalCertificateFailed207,
                "exact raw balance overflow",
            )
        })?;
    }
    let mut meta = Metadata::new();
    let mut enc = Encoder::new_frame(*b"PIMR", 1);
    for input in &inputs {
        inventory::put_envelope(&mut enc, input);
        meta.include(input)?;
    }
    inventory::put_source_envelope(&mut enc, error_model, 6);
    meta.include(error_model)?;
    let covariance = model.covariance.map(|v| v.value());
    let result = reconcile_linear(&observations, &covariance, &BALANCE_ROW, &[0.0])?;
    if (result.raw_residuals[0] - raw_exact.to_f64()).abs() > model.closure_tolerance_kg.value() {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "binary64 raw residual loses the original exact imbalance",
        ));
    }
    for i in 0..5 {
        if ((result.estimates[i] - observations[i]) - result.adjustments[i]).abs()
            > precision_allowance[i]
        {
            return Err(fail(
                RefusalCode::NumericalCertificateFailed207,
                "fitted estimate cannot represent its adjustment within the declared numerical budget",
            ));
        }
    }
    let diagnostics = InventoryReconciliation {
        scope: model.scope.clone(),
        observed_kg: observations.map(Kilograms::new),
        estimates_kg: core::array::from_fn(|i| Kilograms::new(result.estimates[i])),
        adjustments_kg: core::array::from_fn(|i| Kilograms::new(result.adjustments[i])),
        posterior_covariance: core::array::from_fn(|i| {
            KilogramsSquared::new(result.posterior_covariance[i])
        }),
        normalized_adjustments: core::array::from_fn(|i| result.normalized_adjustments[i]),
        raw_residual_kg: Kilograms::new(result.raw_residuals[0]),
        raw_residual_exact_kg: raw_exact,
        reconciled_residual_kg: Kilograms::new(result.reconciled_residuals[0]),
        chi_square: result.chi_square,
        degrees_of_freedom: result.degrees_of_freedom,
    };
    let rejection = if result.estimates.iter().any(|v| *v < 0.0) {
        Some(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "fitted inventory/flow is negative; Gaussian model requires review",
        ))
    } else if result.reconciled_residuals[0].abs() > model.closure_tolerance_kg.value() {
        Some(fail(
            RefusalCode::Conservation101,
            "fitted residual exceeds declared kg closure allowance",
        ))
    } else if result.chi_square > model.global_chi_square_limit {
        Some(fail(
            RefusalCode::Conservation101,
            format!(
                "global chi-square {} exceeds declared limit {}; all observations retained for investigation",
                result.chi_square, model.global_chi_square_limit
            ),
        ))
    } else if let Some(limit) = model.normalized_adjustment_limit
        && result
            .normalized_adjustments
            .iter()
            .flatten()
            .any(|v| v.abs() > limit)
    {
        Some(fail(
            RefusalCode::Conservation101,
            format!(
                "normalized adjustment exceeds declared limit {limit}; correlated diagnostics do not identify a unique faulty instrument"
            ),
        ))
    } else {
        None
    };
    for value in result
        .estimates
        .iter()
        .chain(&result.adjustments)
        .chain(&result.posterior_covariance)
        .chain(&result.raw_residuals)
        .chain(&result.reconciled_residuals)
    {
        enc.put_u64(value.to_bits());
    }
    enc.put_u64(result.chi_square.to_bits());
    enc.put_u64(result.degrees_of_freedom as u64);
    for value in &result.normalized_adjustments {
        enc.put_u8(u8::from(value.is_some()));
        if let Some(v) = value {
            enc.put_u64(v.to_bits());
        }
    }
    enc.put_u8(u8::from(rejection.is_some()));
    if let Some(refusal) = &rejection {
        enc.put_str(refusal.code);
        enc.put_str(&refusal.what);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    if let Some(refusal) = rejection {
        return Ok(InventoryReconciliationOutcome::Rejected(Box::new(
            RejectedInventoryReconciliation {
                inputs,
                error_model: error_model.clone(),
                diagnostics,
                refusal,
                digest,
            },
        )));
    }
    let claim = meta.into_claim(OUTPUT_METRIC, diagnostics, &digest);
    Ok(InventoryReconciliationOutcome::Admitted(Box::new(
        ReconciledInventoryReceipt {
            inputs,
            error_model: error_model.clone(),
            claim,
            digest,
        },
    )))
}

#[cfg(test)]
mod tests;
