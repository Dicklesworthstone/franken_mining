//! DESIGN: exact inventory accounting, conditional on the declared complete
//! population of boundary flows. Opening + feed = product + other outflow +
//! closing; a residual is refused, never converted into a loss or plug stream.
//! Stocktakes are instantaneous; flows span the reporting interval. All masses
//! refer to the same elemental measurand in all phases, in kilograms, at the
//! full physical boundary (not an ownership-attributed share).
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{
    Color, EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode,
    color::validate_color_payload,
};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, UnitsError};

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);

/// Metric identity required on every admitted source projection.
pub const INPUT_METRIC: &str = "plant.inventory.input/1";
/// Metric identity of an exactly closed interval; not a recovery forecast.
pub const OUTPUT_METRIC: &str = "plant.inventory.closed_interval/1";
/// Unit and scope interpretation bound into every input's semantic identity.
pub const MASS_BASIS: &str = "kg-total-element/all-phases/full-physical-boundary";
/// The arithmetic does not establish the completeness or accuracy of its inputs.
pub const NO_CLAIM: &str = "Exact closure conditional on the declared complete boundary population and supplied nominal masses. No measurement-error reconciliation, source audit, recovery estimate, RTD fit, forecast or decision authorization is supplied.";

/// The common physical accounting support of all six input roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceScope {
    /// Stable plant-boundary identity, including its version after modifications.
    pub boundary: String,
    /// Reporting interval between opening and closing stocktake instants.
    pub interval: ValidTime,
    /// Case-sensitive elemental symbol, for example `Au`, `Cu` or `S`.
    /// This is an identity, not a chemical-speciation conversion.
    pub element: String,
    /// Source measurand identity, for example `total-Au/all-phases`.
    /// Different analytical/speciation definitions never combine implicitly.
    pub measurand: String,
}

/// Exactly one of each role is required, including explicit zero totals.
/// Feed and outflows are already integrated total-element masses, not ore
/// tonnes, grades, concentrations, rates or ownership-attributed quantities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceValue {
    /// Source-backed declaration that all boundary flows and held inventory are
    /// represented. Unrecorded losses cannot be assigned a balancing plug.
    Population { complete: bool },
    /// Total element held inside the boundary at the interval's start, in kg.
    OpeningInventory(ExactQty),
    /// All incoming boundary flows over the interval, in kg of the element.
    Feed(ExactQty),
    /// Product leaving the boundary over the interval, in kg of the element.
    Product(ExactQty),
    /// All other outgoing boundary flows, including tailings, purge, spill and
    /// dust if present, in kg of the element. Each must already be accounted for.
    OtherOutflow(ExactQty),
    /// Total element held inside the boundary at the interval's end, in kg.
    ClosingInventory(ExactQty),
}

impl BalanceValue {
    fn key(self) -> u8 {
        match self {
            Self::Population { .. } => 0,
            Self::OpeningInventory(_) => 1,
            Self::Feed(_) => 2,
            Self::Product(_) => 3,
            Self::OtherOutflow(_) => 4,
            Self::ClosingInventory(_) => 5,
        }
    }

    fn mass(self) -> Option<ExactQty> {
        match self {
            Self::Population { .. } => None,
            Self::OpeningInventory(q)
            | Self::Feed(q)
            | Self::Product(q)
            | Self::OtherOutflow(q)
            | Self::ClosingInventory(q) => Some(q),
        }
    }

    /// Population declarations are non-additive; all five mass totals are extensive.
    #[must_use]
    pub const fn additivity(self) -> AdditivityClass {
        match self {
            Self::Population { .. } => AdditivityClass::NonAdditive,
            _ => AdditivityClass::ExtensiveAdditive,
        }
    }

    fn expected_time(self, interval: ValidTime) -> ValidTime {
        match self {
            Self::OpeningInventory(_) => ValidTime::instant(interval.start_ns),
            Self::ClosingInventory(_) => ValidTime::instant(interval.end_ns),
            _ => interval,
        }
    }
}

/// A typed projection of one source observation or completeness declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetalBalanceInput {
    pub scope: BalanceScope,
    /// Source observation's physical support: the exact reporting interval for
    /// flows/declaration, or the correct endpoint instant for a stocktake.
    pub physical_time: ValidTime,
    pub value: BalanceValue,
}

impl MetalBalanceInput {
    /// Semantic identity to pass as `Claim::new`'s canonical value tag. Admission
    /// recomputes it, because generic Claim ids do not directly bind typed values.
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PLNI", 1);
        put_scope(&mut enc, &self.scope);
        put_time(&mut enc, self.physical_time);
        enc.put_u8(self.value.key());
        match self.value {
            BalanceValue::Population { complete } => enc.put_u8(u8::from(complete)),
            BalanceValue::OpeningInventory(q)
            | BalanceValue::Feed(q)
            | BalanceValue::Product(q)
            | BalanceValue::OtherOutflow(q)
            | BalanceValue::ClosingInventory(q) => put_qty(&mut enc, q),
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Exact nominal totals for one closed boundary interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetalBalanceSummary {
    pub scope: BalanceScope,
    pub opening_inventory_kg: ExactQty,
    pub feed_kg: ExactQty,
    pub product_kg: ExactQty,
    pub other_outflow_kg: ExactQty,
    pub closing_inventory_kg: ExactQty,
    /// Closing minus opening; negative values are physical inventory drawdown.
    pub inventory_change_kg: ExactQty,
    /// Opening + feed - product - other outflow - closing; exactly zero on success.
    pub residual_kg: ExactQty,
    /// Diagnostic only: product / same-interval feed, with no hold-up correction.
    /// May exceed one during drawdown; absent when feed is zero. Not recovery.
    pub product_to_feed_ratio: Option<ExactQty>,
}

/// Immutable admitted sources and their derived balance claim. The receipt is
/// issued only after exact closure. Changing a cloned Claim cannot alter it.
#[derive(Debug, Clone, PartialEq)]
pub struct MetalBalanceReceipt {
    inputs: Vec<Claim<MetalBalanceInput>>,
    claim: Claim<MetalBalanceSummary>,
    digest: String,
}

impl MetalBalanceReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<MetalBalanceInput>] {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &MetalBalanceSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<MetalBalanceSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant inventory: {}", detail.into()))
}
fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact quantity arithmetic is unrepresentable",
        )
    })
}
fn named(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && !text.chars().any(char::is_control)
}
fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}
fn put_time(enc: &mut Encoder, time: ValidTime) {
    enc.put_i64(time.start_ns);
    enc.put_i64(time.end_ns);
}
fn put_scope(enc: &mut Encoder, scope: &BalanceScope) {
    enc.put_str(&scope.boundary);
    put_time(enc, scope.interval);
    enc.put_str(&scope.element);
    enc.put_str(&scope.measurand);
    enc.put_str(MASS_BASIS);
}

/// Admit a source's common nominal envelope; callers validate their typed payload.
/// The semantic tag is recomputed by that payload's owner, not read from the claim.
pub(crate) fn admit_source<T>(
    input: &Claim<T>,
    expected_metric: &str,
    semantic_tag: &str,
    expected_additivity: AdditivityClass,
) -> Result<()> {
    admit_source_at_support(
        input,
        expected_metric,
        semantic_tag,
        expected_additivity,
        SpatialSupport::PeriodAggregate,
    )
}

/// The raw-lot projection keeps PlantLot support until its declared aggregation.
pub(crate) fn admit_source_at_support<T>(
    input: &Claim<T>,
    expected_metric: &str,
    semantic_tag: &str,
    expected_additivity: AdditivityClass,
    expected_support: SpatialSupport,
) -> Result<()> {
    input.check_id()?;
    if input.metric_key != expected_metric || input.value_tag() != semantic_tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "input value or metric differs from its bound semantic identity",
        ));
    }
    if input.support != expected_support {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "source support does not match the declared observation role",
        ));
    }
    if input.additivity != expected_additivity {
        return Err(bad(
            RefusalCode::NonAdditiveAverage103,
            "source additivity does not match the declared input role",
        ));
    }
    if input.stale || input.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale source input"));
    }
    if input.calibration_failed {
        return Err(bad(
            RefusalCode::CalibrationRequired205,
            "failed-calibration input cannot supply a numeric balance",
        ));
    }
    if !matches!(
        input.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || input.representation != Representation::Deterministic
    {
        return Err(bad(
            RefusalCode::NotObservable202,
            "requires usable deterministic nominal totals; uncertain or unavailable inputs need explicit reconciliation",
        ));
    }
    if input.anchors.is_empty()
        || input
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every mass and the completeness declaration need a source artifact digest and locator",
        ));
    }
    if input.uu_terms.iter().any(|term| {
        !named(&term.scenario_id)
            || !named(&term.description)
            || !named(&term.trigger_indicator)
            || !term.downside_exposure_usd_m.is_finite()
            || term.downside_exposure_usd_m < 0.0
    }) {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "invalid supplied unknown-unknown metadata",
        ));
    }
    if let Ok(color) = input.color() {
        validate_color_payload(&color).map_err(|_| {
            bad(
                RefusalCode::CalibrationRequired205,
                "invalid input colour metadata",
            )
        })?;
    }
    Ok(())
}

pub(crate) fn admit(input: &Claim<MetalBalanceInput>) -> Result<()> {
    admit_source(
        input,
        INPUT_METRIC,
        &input.value.canonical_tag(),
        input.value.value.additivity(),
    )?;
    let source = &input.value;
    let scope = &source.scope;
    let symbol = scope.element.as_bytes();
    if !named(&scope.boundary)
        || !named(&scope.measurand)
        || !(symbol.len() == 1 || symbol.len() == 2)
        || !symbol[0].is_ascii_uppercase()
        || (symbol.len() == 2 && !symbol[1].is_ascii_lowercase())
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare a boundary, case-sensitive elemental symbol and conserved measurand identity",
        ));
    }
    if scope.interval.start_ns >= scope.interval.end_ns {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "the reporting interval must have positive duration",
        ));
    }
    if source.physical_time != source.value.expected_time(scope.interval) {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "flow totals need the exact interval; opening/closing stocks need the corresponding endpoint",
        ));
    }
    if source.value.mass().is_some_and(|q| q.signum() < 0) {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "individual physical stocks and boundary flow totals must be nonnegative",
        ));
    }
    Ok(())
}

// Generic Claim bodies omit these fields. Binding the admitted envelope keeps
// calibration, support, uncertainty and supersession changes visible in receipts.
pub(crate) fn put_source_envelope<T>(enc: &mut Encoder, input: &Claim<T>, role_tag: u8) {
    enc.put_bytes(&input.canonical_body());
    match input.support {
        SpatialSupport::PeriodAggregate => enc.put_str("Deterministic/PeriodAggregate"),
        SpatialSupport::PlantLot => enc.put_str("Deterministic/PlantLot"),
        SpatialSupport::PointCore => enc.put_str("Deterministic/PointCore"),
        SpatialSupport::MiningPolygon => enc.put_str("Deterministic/MiningPolygon"),
        SpatialSupport::Composite { length_mm } => {
            enc.put_str("Deterministic/Composite");
            enc.put_u32(length_mm);
        }
        SpatialSupport::SmuBlock {
            dx_cm,
            dy_cm,
            dz_cm,
        } => {
            enc.put_str("Deterministic/SmuBlock");
            enc.put_u32(dx_cm);
            enc.put_u32(dy_cm);
            enc.put_u32(dz_cm);
        }
    }
    enc.put_u8(role_tag);
    enc.put_u8(u8::from(input.stale));
    enc.put_u8(u8::from(input.calibration_failed));
    enc.put_str(
        &input
            .supersedes
            .map_or_else(String::new, |id| id.to_string()),
    );
    enc.put_u64(input.uu_terms.len() as u64);
    for term in &input.uu_terms {
        enc.put_str(&term.scenario_id);
        enc.put_str(&term.description);
        enc.put_u64(term.downside_exposure_usd_m.to_bits());
        enc.put_str(&term.trigger_indicator);
    }
    match input.color() {
        Err(refusal) => {
            enc.put_u8(0);
            enc.put_str(refusal.code);
        }
        Ok(Color::Verified { lo, hi }) => {
            enc.put_u8(1);
            enc.put_u64(lo.to_bits());
            enc.put_u64(hi.to_bits());
        }
        Ok(Color::Validated { regime, dataset }) => {
            enc.put_u8(2);
            enc.put_str(&dataset);
            enc.put_u64(regime.bounds().len() as u64);
            for (axis, (lo, hi)) in regime.bounds() {
                enc.put_str(axis);
                enc.put_u64(lo.to_bits());
                enc.put_u64(hi.to_bits());
            }
        }
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => {
            enc.put_u8(3);
            enc.put_str(&estimator);
            enc.put_u64(dispersion.to_bits());
        }
    }
}

pub(crate) fn put_envelope(enc: &mut Encoder, input: &Claim<MetalBalanceInput>) {
    put_source_envelope(enc, input, input.value.value.key());
}

/// Shared nominal conservation identity for elemental and carrier balances.
/// Callers supply already admitted nonnegative masses in one consistent unit.
pub(crate) fn conservation_residual(
    opening: ExactQty,
    feed: ExactQty,
    outflow: ExactQty,
    closing: ExactQty,
) -> Result<ExactQty> {
    let available = exact(opening.checked_add(feed))?;
    exact(exact(available.checked_sub(outflow))?.checked_sub(closing))
}

/// Account for one complete interval without fitting, plugging, tolerance
/// selection or implicit unit/basis conversion. Inputs may be in any order.
/// On a nonzero residual, `FMINE-REF-101` reports the signed exact quantity and
/// no closed receipt is returned. Arithmetic precision is not measurement accuracy.
pub fn account_interval(inputs: &[Claim<MetalBalanceInput>]) -> Result<MetalBalanceReceipt> {
    if inputs.len() != 6 {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "requires exactly one population declaration, opening inventory, feed, product, other outflow and closing inventory, including explicit zero totals",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by_key(|input| input.value.value.key());
    for (key, input) in inputs.iter().enumerate() {
        admit(input)?;
        if input.value.value.key() as usize != key {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate role or missing boundary-flow/stocktake role",
            ));
        }
        if input.value.scope != inputs[0].value.scope {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                "inputs disagree on boundary version, elemental measurand or reporting interval",
            ));
        }
    }
    if inputs[0].value.value != (BalanceValue::Population { complete: true }) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "the source population must explicitly declare all flows and held inventory complete",
        ));
    }
    // The exact role check above establishes this order; no missing mass defaults to zero.
    let masses: Vec<_> = inputs
        .iter()
        .filter_map(|input| input.value.value.mass())
        .collect();
    let [opening, feed, product, other, closing] = masses.as_slice() else {
        return Err(bad(RefusalCode::NoClaimInDomain201, "missing mass role"));
    };
    let discharge = exact(product.checked_add(*other))?;
    let residual = conservation_residual(*opening, *feed, discharge, *closing)?;
    if residual != ZERO {
        return Err(bad(
            RefusalCode::Conservation101,
            format!(
                "opening + feed - product - other outflow - closing = {}/{} kg {}; exact closure is required; no balancing stream was inserted",
                residual.num(),
                residual.den(),
                inputs[0].value.scope.element,
            ),
        ));
    }
    let summary = MetalBalanceSummary {
        scope: inputs[0].value.scope.clone(),
        opening_inventory_kg: *opening,
        feed_kg: *feed,
        product_kg: *product,
        other_outflow_kg: *other,
        closing_inventory_kg: *closing,
        inventory_change_kg: exact(closing.checked_sub(*opening))?,
        residual_kg: residual,
        product_to_feed_ratio: if *feed == ZERO {
            None
        } else {
            Some(exact(product.checked_div(*feed))?)
        },
    };

    let mut enc = Encoder::new_frame(*b"PLNR", 1);
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut uu = BTreeMap::new();
    let mut parents = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    for input in &inputs {
        put_envelope(&mut enc, input);
        evidence = evidence.meet(input.evidence);
        epistemic = epistemic.and(input.epistemic);
        state = state.and(input.state);
        confidentiality = confidentiality.join(&input.confidentiality);
        parents.insert(input.claim_id);
        anchors.extend(input.anchors.iter().cloned());
        for term in &input.uu_terms {
            if let Some(previous) = uu.insert(term.scenario_id.clone(), term.clone())
                && previous != *term
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting definitions for the same unknown-unknown scenario",
                ));
            }
        }
    }
    put_qty(&mut enc, summary.inventory_change_kg);
    put_qty(&mut enc, summary.residual_kg);
    enc.put_u8(u8::from(summary.product_to_feed_ratio.is_some()));
    if let Some(ratio) = summary.product_to_feed_ratio {
        put_qty(&mut enc, ratio);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &digest,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    );
    Ok(MetalBalanceReceipt {
        inputs,
        claim,
        digest,
    })
}

#[cfg(test)]
mod tests;
