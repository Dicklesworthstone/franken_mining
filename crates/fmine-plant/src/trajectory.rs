//! DESIGN: bounded joint linear inventory estimation across contiguous periods
//! (plan §§20.2–20.3, 20.8; BD-154/177). Each boundary stock is ONE coordinate,
//! shared by its adjoining periods. Only genuine measurements have covariance
//! rows; unmeasured stocks have no fabricated observations or process priors.
//! Determinism: DeterministicPerIsa

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, ClaimId};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::reconciliation::{
    partial::{PartialReconciliation, reconcile_with_unmeasured},
    reconcile_linear,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

use crate::{
    flowsheet::reconciliation::unmeasured::GrossErrorDiagnostic,
    inventory::{self, BalanceScope},
    reconciliation::{Kilograms, KilogramsSquared, Metadata, nominal_with_precision_budget},
};

mod render;
pub mod worksheet;

type Result<T> = std::result::Result<T, Refusal>;
pub const MAX_PERIODS: usize = 32;
pub const MAX_COORDINATES: usize = 3 * MAX_PERIODS + 1;
pub const INPUT_METRIC: &str = "plant.trajectory.input/1";
pub const ERROR_MODEL_METRIC: &str = "plant.trajectory.error_model/1";
pub const OUTPUT_METRIC: &str = "plant.trajectory.reconciled_inventory/1";

/// One conserved elemental total at one full physical storage boundary. Inflow
/// and outflow are complete integrated totals, never rates or net plug rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrajectoryValue {
    Population {
        /// Strictly increasing endpoints of 2..32 contiguous periods.
        boundaries_ns: Vec<i64>,
        /// Every unmeasured endpoint is explicit, including an unmeasured final stock.
        unmeasured_stocktakes: BTreeSet<i64>,
        complete: bool,
    },
    Stocktake(ExactQty),
    Inflow(ExactQty),
    Outflow(ExactQty),
}
impl TrajectoryValue {
    pub const fn additivity(&self) -> AdditivityClass {
        match self {
            Self::Population { .. } => AdditivityClass::NonAdditive,
            _ => AdditivityClass::ExtensiveAdditive,
        }
    }
    fn mass(&self) -> Option<ExactQty> {
        match self {
            Self::Population { .. } => None,
            Self::Stocktake(q) | Self::Inflow(q) | Self::Outflow(q) => Some(*q),
        }
    }
    fn role(&self) -> u8 {
        match self {
            Self::Population { .. } => 0,
            Self::Stocktake(_) => 1,
            Self::Inflow(_) => 2,
            Self::Outflow(_) => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrajectoryInput {
    pub scope: BalanceScope,
    /// Whole scope for the population, exact period for a flow, or endpoint instant.
    pub physical_time: ValidTime,
    pub value: TrajectoryValue,
}
impl TrajectoryInput {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PTIN", 1);
        put_scope(&mut enc, &self.scope);
        put_time(&mut enc, self.physical_time);
        enc.put_u8(self.value.role());
        match &self.value {
            TrajectoryValue::Population {
                boundaries_ns,
                unmeasured_stocktakes,
                complete,
            } => {
                enc.put_u8(u8::from(*complete));
                enc.put_u64(boundaries_ns.len() as u64);
                for time in boundaries_ns {
                    enc.put_i64(*time);
                }
                enc.put_u64(unmeasured_stocktakes.len() as u64);
                for time in unmeasured_stocktakes {
                    enc.put_i64(*time);
                }
            }
            _ => {
                if let Some(q) = self.value.mass() {
                    put_qty(&mut enc, q);
                }
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
    fn key(&self) -> (u8, i64, i64) {
        (
            self.value.role(),
            self.physical_time.start_ns,
            self.physical_time.end_ns,
        )
    }
    fn location(&self) -> Option<TrajectoryLocation> {
        let t = self.physical_time;
        match self.value {
            TrajectoryValue::Population { .. } => None,
            TrajectoryValue::Stocktake(_) => Some(TrajectoryLocation::Stocktake(t.start_ns)),
            TrajectoryValue::Inflow(_) => Some(TrajectoryLocation::Inflow {
                start_ns: t.start_ns,
                end_ns: t.end_ns,
            }),
            TrajectoryValue::Outflow(_) => Some(TrajectoryLocation::Outflow {
                start_ns: t.start_ns,
                end_ns: t.end_ns,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrajectoryLocation {
    Stocktake(i64),
    Inflow { start_ns: i64, end_ns: i64 },
    Outflow { start_ns: i64, end_ns: i64 },
}
impl TrajectoryLocation {
    pub fn worksheet_key(&self) -> String {
        match self {
            Self::Stocktake(t) => format!("stock/{t}"),
            Self::Inflow { start_ns, end_ns } => format!("inflow/{start_ns}/{end_ns}"),
            Self::Outflow { start_ns, end_ns } => format!("outflow/{start_ns}/{end_ns}"),
        }
    }
    fn coefficient(&self, period: ValidTime) -> i8 {
        match self {
            Self::Stocktake(t) if *t == period.start_ns => 1,
            Self::Stocktake(t) if *t == period.end_ns => -1,
            Self::Inflow { start_ns, end_ns }
                if (*start_ns, *end_ns) == (period.start_ns, period.end_ns) =>
            {
                1
            }
            Self::Outflow { start_ns, end_ns }
                if (*start_ns, *end_ns) == (period.start_ns, period.end_ns) =>
            {
                -1
            }
            _ => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrajectoryObservation {
    pub source: ClaimId,
    pub location: TrajectoryLocation,
}

/// Declared conditional Gaussian model, not estimated sensor accuracy. The
/// entire full matrix covers genuine observations only, including cross-period
/// terms. Its coordinate order must equal `observation_order` exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryErrorModel {
    pub scope: BalanceScope,
    pub declaration_id: ClaimId,
    pub observations: Vec<TrajectoryObservation>,
    pub covariance: Vec<KilogramsSquared>,
    pub error_model_basis: String,
    pub unbiased_gaussian_errors: bool,
    pub global_chi_square_limit: f64,
    pub normalized_adjustment_limit: Option<f64>,
    pub closure_tolerance_kg: Kilograms,
}
impl TrajectoryErrorModel {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PTEM", 1);
        put_scope(&mut enc, &self.scope);
        enc.put_str(&self.declaration_id.to_string());
        enc.put_str(&self.error_model_basis);
        enc.put_u64(self.observations.len() as u64);
        for key in &self.observations {
            enc.put_str(&key.source.to_string());
            enc.put_str(&key.location.worksheet_key());
        }
        enc.put_u64(self.covariance.len() as u64);
        for value in &self.covariance {
            enc.put_u64(value.value().to_bits());
        }
        enc.put_u8(u8::from(self.unbiased_gaussian_errors));
        enc.put_u64(self.global_chi_square_limit.to_bits());
        put_optional(&mut enc, self.normalized_adjustment_limit);
        enc.put_u64(self.closure_tolerance_kg.value().to_bits());
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeriodConservation {
    pub interval: ValidTime,
    /// With a missing endpoint this is only the observed contribution, not an imbalance.
    pub observed_contribution_exact_kg: ExactQty,
    pub has_unmeasured_terms: bool,
    pub reconciled_residual_kg: Kilograms,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryTrajectory {
    pub scope: BalanceScope,
    pub boundaries_ns: Vec<i64>,
    pub observations: Vec<TrajectoryObservation>,
    pub unmeasured_stocktakes_ns: Vec<i64>,
    pub observed_kg: Vec<Kilograms>,
    /// Measured coordinates in observation order, then latent stocks by instant.
    pub estimates_kg: Vec<Kilograms>,
    pub measured_adjustments_kg: Vec<Kilograms>,
    /// Full row-major joint covariance in the same measured-then-latent order.
    pub posterior_covariance: Vec<KilogramsSquared>,
    pub measured_adjustment_covariance: Vec<KilogramsSquared>,
    pub normalized_adjustments: Vec<Option<f64>>,
    pub conservation: Vec<PeriodConservation>,
    pub gross_error: GrossErrorDiagnostic,
}
impl InventoryTrajectory {
    pub fn stock_coordinate_index(&self, at_ns: i64) -> Option<usize> {
        self.observations
            .iter()
            .position(|key| key.location == TrajectoryLocation::Stocktake(at_ns))
            .or_else(|| {
                self.unmeasured_stocktakes_ns
                    .iter()
                    .position(|time| *time == at_ns)
                    .map(|i| i + self.observations.len())
            })
    }
}

/// Immutable original evidence and admitted conditional estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct TrajectoryReceipt {
    inputs: Vec<Claim<TrajectoryInput>>,
    error_model: Claim<TrajectoryErrorModel>,
    claim: Claim<InventoryTrajectory>,
    digest: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RejectedTrajectory {
    inputs: Vec<Claim<TrajectoryInput>>,
    error_model: Claim<TrajectoryErrorModel>,
    diagnostics: InventoryTrajectory,
    refusal: Refusal,
    digest: String,
}
#[derive(Debug, Clone, PartialEq)]
pub enum TrajectoryOutcome {
    Admitted(Box<TrajectoryReceipt>),
    Rejected(Box<RejectedTrajectory>),
}
impl TrajectoryOutcome {
    pub fn inputs(&self) -> &[Claim<TrajectoryInput>] {
        match self {
            Self::Admitted(r) => &r.inputs,
            Self::Rejected(r) => &r.inputs,
        }
    }
    pub fn error_model(&self) -> &Claim<TrajectoryErrorModel> {
        match self {
            Self::Admitted(r) => &r.error_model,
            Self::Rejected(r) => &r.error_model,
        }
    }
    pub fn diagnostics(&self) -> &InventoryTrajectory {
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
    pub fn admitted_claim(&self) -> Result<&Claim<InventoryTrajectory>> {
        match self {
            Self::Admitted(r) => Ok(&r.claim),
            Self::Rejected(r) => Err(r.refusal.clone()),
        }
    }
}

fn fail(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("inventory trajectory: {}", detail.into()))
}
fn numerical(detail: &str) -> Refusal {
    fail(RefusalCode::NumericalCertificateFailed207, detail)
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn put_time(enc: &mut Encoder, time: ValidTime) {
    enc.put_i64(time.start_ns);
    enc.put_i64(time.end_ns);
}
fn put_qty(enc: &mut Encoder, quantity: ExactQty) {
    enc.put_i128(quantity.num());
    enc.put_i128(quantity.den());
}
fn put_scope(enc: &mut Encoder, scope: &BalanceScope) {
    enc.put_str(&scope.boundary);
    enc.put_str(&scope.element);
    enc.put_str(&scope.measurand);
    enc.put_str(inventory::MASS_BASIS);
    put_time(enc, scope.interval);
}
fn put_optional(enc: &mut Encoder, value: Option<f64>) {
    enc.put_u8(u8::from(value.is_some()));
    if let Some(value) = value {
        enc.put_u64(value.to_bits());
    }
}

struct AdmittedInputs<'a> {
    inputs: Vec<&'a Claim<TrajectoryInput>>,
    boundaries: &'a [i64],
    missing: &'a BTreeSet<i64>,
    measurements: BTreeMap<TrajectoryLocation, &'a Claim<TrajectoryInput>>,
}
impl AdmittedInputs<'_> {
    fn order(&self) -> Vec<TrajectoryObservation> {
        self.measurements
            .iter()
            .map(|(location, c)| TrajectoryObservation {
                source: c.claim_id,
                location: location.clone(),
            })
            .collect()
    }
}

fn admit_inputs(inputs: &[Claim<TrajectoryInput>]) -> Result<AdmittedInputs<'_>> {
    if inputs.is_empty() || inputs.len() > MAX_COORDINATES + 1 {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "bounded population and genuine measurements required",
        ));
    }
    let mut ordered: Vec<_> = inputs.iter().collect();
    ordered.sort_by_key(|c| c.value.key());
    let first = ordered[0];
    let TrajectoryValue::Population {
        boundaries_ns,
        unmeasured_stocktakes,
        complete,
    } = &first.value.value
    else {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "one complete population declaration required",
        ));
    };
    if !(3..=MAX_PERIODS + 1).contains(&boundaries_ns.len())
        || unmeasured_stocktakes.len() > MAX_PERIODS + 1
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "trajectory requires 2..32 periods",
        ));
    }
    let scope = &first.value.scope;
    let symbol = scope.element.as_bytes();
    if !named(&scope.boundary)
        || !named(&scope.measurand)
        || !(symbol.len() == 1 || symbol.len() == 2)
        || !symbol[0].is_ascii_uppercase()
        || (symbol.len() == 2 && !symbol[1].is_ascii_lowercase())
    {
        return Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare one storage boundary, elemental symbol and conserved total measurand",
        ));
    }
    if boundaries_ns.windows(2).any(|w| w[0] >= w[1])
        || boundaries_ns[0] != scope.interval.start_ns
        || boundaries_ns[boundaries_ns.len() - 1] != scope.interval.end_ns
        || first.value.physical_time != scope.interval
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "strictly increasing shared boundaries must exactly tile the declared scope",
        ));
    }
    if !complete {
        return Err(fail(
            RefusalCode::NoClaimInDomain201,
            "all inventory and inflow/outflow totals must be declared complete",
        ));
    }
    if unmeasured_stocktakes
        .iter()
        .any(|t| !boundaries_ns.contains(t))
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "unmeasured stocktake is not a declared period boundary",
        ));
    }
    let mut measurements = BTreeMap::new();
    for (i, input) in ordered.iter().enumerate() {
        if i > 0 && matches!(input.value.value, TrajectoryValue::Population { .. }) {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "duplicate population declaration",
            ));
        }
        inventory::admit_source(
            input,
            INPUT_METRIC,
            &input.value.canonical_tag(),
            input.value.value.additivity(),
        )?;
        if input.value.scope != *scope {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "measurements use different physical scopes or measurands",
            ));
        }
        let time = input.value.physical_time;
        if let Some(location) = input.value.location() {
            let valid_time = match location {
                TrajectoryLocation::Stocktake(t) => {
                    time.start_ns == time.end_ns && boundaries_ns.contains(&t)
                }
                _ => boundaries_ns
                    .windows(2)
                    .any(|w| time.start_ns == w[0] && time.end_ns == w[1]),
            };
            if !valid_time {
                return Err(fail(
                    RefusalCode::SupportMismatch102,
                    "stocks need exact boundary instants and flows need an exact declared period",
                ));
            }
            if input.value.value.mass().is_some_and(|q| q.signum() < 0) {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "observed stocks and flows must be nonnegative total elemental kg",
                ));
            }
            if measurements.insert(location, *input).is_some() {
                return Err(fail(
                    RefusalCode::BasisMismatch105,
                    "duplicate measurement at one shared coordinate",
                ));
            }
        }
    }
    for t in boundaries_ns {
        let observed = measurements.contains_key(&TrajectoryLocation::Stocktake(*t));
        let latent = unmeasured_stocktakes.contains(t);
        if observed == latent {
            return Err(fail(
                RefusalCode::BasisMismatch105,
                "each boundary stock must be measured exactly once or explicitly unmeasured",
            ));
        }
    }
    for w in boundaries_ns.windows(2) {
        for location in [
            TrajectoryLocation::Inflow {
                start_ns: w[0],
                end_ns: w[1],
            },
            TrajectoryLocation::Outflow {
                start_ns: w[0],
                end_ns: w[1],
            },
        ] {
            if !measurements.contains_key(&location) {
                return Err(fail(
                    RefusalCode::NoClaimInDomain201,
                    "every period needs both genuine inflow and outflow totals, including explicit zeros",
                ));
            }
        }
    }
    if unmeasured_stocktakes.len() == boundaries_ns.len() {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "no measured stocktake anchors the trajectory's common inventory offset",
        ));
    }
    Ok(AdmittedInputs {
        inputs: ordered,
        boundaries: boundaries_ns,
        missing: unmeasured_stocktakes,
        measurements,
    })
}

/// Admits temporal/population semantics and returns the only valid covariance order.
pub fn observation_order(inputs: &[Claim<TrajectoryInput>]) -> Result<Vec<TrajectoryObservation>> {
    Ok(admit_inputs(inputs)?.order())
}

/// Condition all periods simultaneously. A later supplied stocktake creates a new
/// receipt and can update earlier stocks; this API never edits an earlier receipt
/// or claims that future evidence was available at an earlier decision time.
pub fn reconcile_trajectory(
    inputs: &[Claim<TrajectoryInput>],
    error_model: &Claim<TrajectoryErrorModel>,
) -> Result<TrajectoryOutcome> {
    let admitted = admit_inputs(inputs)?;
    let model = &error_model.value;
    let observations = admitted.order();
    let n = observations.len();
    let p = admitted.missing.len();
    let m = admitted.boundaries.len() - 1;
    if model.observations.len() != n || model.covariance.len() != n * n {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "full covariance must cover exactly the genuine measurements",
        ));
    }
    inventory::admit_source(
        error_model,
        ERROR_MODEL_METRIC,
        &model.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if model.scope != admitted.inputs[0].value.scope
        || model.declaration_id != admitted.inputs[0].claim_id
        || model.observations != observations
    {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "error model must bind the exact population, measurement identities and canonical order",
        ));
    }
    if !named(&model.error_model_basis)
        || !model.unbiased_gaussian_errors
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
            "declare error basis, unbiased Gaussian errors, positive diagnostic limits and nonnegative finite closure allowance",
        ));
    }
    let exact: Vec<_> = admitted
        .measurements
        .values()
        .filter_map(|c| c.value.value.mass())
        .collect();
    let mut observed = Vec::with_capacity(n);
    let mut precision = Vec::with_capacity(n);
    for (i, q) in exact.iter().enumerate() {
        let (value, budget) = nominal_with_precision_budget(
            *q,
            model.covariance[i * n + i],
            model.closure_tolerance_kg,
            n + p,
        )?;
        observed.push(value);
        precision.push(budget);
    }
    let mut measured_matrix = Vec::with_capacity(m * n);
    let mut latent_matrix = Vec::with_capacity(m * p);
    let mut conservation = Vec::with_capacity(m);
    for w in admitted.boundaries.windows(2) {
        let interval = ValidTime {
            start_ns: w[0],
            end_ns: w[1],
        };
        let mut raw = ExactQty::from_integer(0);
        for (i, key) in observations.iter().enumerate() {
            let sign = key.location.coefficient(interval);
            measured_matrix.push(f64::from(sign));
            raw = match sign {
                1 => raw.checked_add(exact[i]),
                -1 => raw.checked_sub(exact[i]),
                _ => Ok(raw),
            }
            .map_err(|_| numerical("exact observed conservation contribution overflows"))?;
        }
        for time in admitted.missing {
            latent_matrix.push(f64::from(
                TrajectoryLocation::Stocktake(*time).coefficient(interval),
            ));
        }
        conservation.push(PeriodConservation {
            interval,
            observed_contribution_exact_kg: raw,
            has_unmeasured_terms: admitted.missing.contains(&w[0])
                || admitted.missing.contains(&w[1]),
            reconciled_residual_kg: Kilograms::new(0.0),
        });
    }
    let covariance: Vec<_> = model.covariance.iter().map(|v| v.value()).collect();
    let targets = vec![0.0; m];
    let fit = if p == 0 {
        let fit = reconcile_linear(&observed, &covariance, &measured_matrix, &targets)?;
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
            &measured_matrix,
            &latent_matrix,
            &targets,
            p,
        )?
    };
    for (i, row) in conservation.iter_mut().enumerate() {
        if (fit.raw_observed_residuals[i] - row.observed_contribution_exact_kg.to_f64()).abs()
            > model.closure_tolerance_kg.value()
        {
            return Err(numerical(
                "binary64 conversion loses the original exact observed contribution",
            ));
        }
        row.reconciled_residual_kg = Kilograms::new(fit.reconciled_residuals[i]);
    }
    for i in 0..n {
        if ((fit.measured_estimates[i] - observed[i]) - fit.measured_adjustments[i]).abs()
            > precision[i]
        {
            return Err(numerical(
                "fitted measurement cannot represent its adjustment within the declared numerical budget",
            ));
        }
    }
    let fitted: Vec<_> = fit
        .measured_estimates
        .iter()
        .chain(&fit.latent_estimates)
        .copied()
        .collect();
    let rejection = if fitted.iter().any(|v| *v < 0.0) {
        Some(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "fitted stock or flow is negative; retain observations and investigate the model",
        ))
    } else if fit
        .reconciled_residuals
        .iter()
        .any(|v| v.abs() > model.closure_tolerance_kg.value())
    {
        Some(fail(
            RefusalCode::Conservation101,
            "a period fails its declared conservation allowance",
        ))
    } else if fit.degrees_of_freedom > 0 && fit.chi_square > model.global_chi_square_limit {
        Some(fail(
            RefusalCode::Conservation101,
            "joint gross-error statistic exceeds the declared limit; every observation is retained",
        ))
    } else if fit.degrees_of_freedom > 0
        && model.normalized_adjustment_limit.is_some_and(|limit| {
            fit.normalized_adjustments
                .iter()
                .flatten()
                .any(|v| v.abs() > limit)
        })
    {
        Some(fail(
            RefusalCode::Conservation101,
            "correlated normalized adjustments exceed the declared simultaneous policy",
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
    let diagnostics = InventoryTrajectory {
        scope: model.scope.clone(),
        boundaries_ns: admitted.boundaries.to_vec(),
        observations,
        unmeasured_stocktakes_ns: admitted.missing.iter().copied().collect(),
        observed_kg: observed.into_iter().map(Kilograms::new).collect(),
        estimates_kg: fitted.into_iter().map(Kilograms::new).collect(),
        measured_adjustments_kg: fit
            .measured_adjustments
            .into_iter()
            .map(Kilograms::new)
            .collect(),
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
        conservation,
        gross_error,
    };
    let mut meta = Metadata::new();
    let mut enc = Encoder::new_frame(*b"PTRR", 1);
    enc.put_u64(admitted.inputs.len() as u64);
    for source in &admitted.inputs {
        inventory::put_source_envelope(&mut enc, *source, source.value.value.role());
        meta.include(*source)?;
    }
    inventory::put_source_envelope(&mut enc, error_model, 4);
    meta.include(error_model)?;
    put_diagnostics(&mut enc, &diagnostics);
    enc.put_u8(u8::from(rejection.is_some()));
    if let Some(refusal) = &rejection {
        enc.put_str(refusal.code);
        enc.put_str(&refusal.what);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let inputs = admitted.inputs.into_iter().cloned().collect();
    if let Some(refusal) = rejection {
        Ok(TrajectoryOutcome::Rejected(Box::new(RejectedTrajectory {
            inputs,
            error_model: error_model.clone(),
            diagnostics,
            refusal,
            digest,
        })))
    } else {
        let claim = meta.into_claim(OUTPUT_METRIC, diagnostics, &digest);
        Ok(TrajectoryOutcome::Admitted(Box::new(TrajectoryReceipt {
            inputs,
            error_model: error_model.clone(),
            claim,
            digest,
        })))
    }
}

fn put_diagnostics(enc: &mut Encoder, value: &InventoryTrajectory) {
    for values in [
        &value.observed_kg,
        &value.estimates_kg,
        &value.measured_adjustments_kg,
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
    for value in &value.normalized_adjustments {
        put_optional(enc, *value);
    }
    for row in &value.conservation {
        put_time(enc, row.interval);
        put_qty(enc, row.observed_contribution_exact_kg);
        enc.put_u8(u8::from(row.has_unmeasured_terms));
        enc.put_u64(row.reconciled_residual_kg.value().to_bits());
    }
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
