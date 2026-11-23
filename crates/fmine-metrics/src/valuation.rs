//! DESIGN: the shared plant realization becomes an immediate cash-margin decision
//! (plan §§9.5, 21.6, 23.4; BD-061/225). Exact arithmetic starts from represented
//! binary64 physical quantities; it does not claim exact physical measurements.
//! Determinism: DeterministicPerIsa (inherits the admitted plant computation).

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{
    AdditivityClass, Claim, ClaimId, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::{anchor::SourceAnchor, codec::Encoder, hash::sha256};
use fmine_decide::card::{
    self, ActionPayoffs, ActionRole, DecisionInputs, EvaluatedCard, WeightSet,
};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{
    EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal, RefusalCode,
};
use fmine_plant::production::ProductionReceipt;
use fmine_units::{Currency, ExactQty as Rat, Money, UnitsError};

pub mod comparison;
pub mod investment;
mod render;
#[cfg(test)]
mod tests;
pub mod worksheet;

pub const INPUT_METRIC: &str = "production.valuation.terms/1";
pub const VALUATION_BASIS: &str = "immediate_one_period_cash_margin";
pub const DO_NOTHING: &str = "do-nothing";
pub const NO_CLAIM: &str = "Conditional immediate one-period USD cash margins over the complete declared shared production and commercial scenarios. Recovered metal is converted to payable sold metal only under the explicit physical-to-commercial assumption. Prices, payability, royalties, costs and do-nothing cash are declarations. No inventory or settlement lag, ownership conversion, tax, discounting, project NPV, calibrated probability, investment suitability, physical feasibility or execution authorization is inferred. Money is rounded once under the declared rule; every unrounded value and rounding difference is retained.";
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

/// The only admitted monetary conversion is explicitly selected in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonetaryRounding {
    NearestTiesEvenFourPlaces,
}

impl MonetaryRounding {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NearestTiesEvenFourPlaces => "nearest_ties_even_0_0001_usd",
        }
    }
}

/// Commercial terms for the same physical state, never independent marginal draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioTerms {
    pub id: String,
    pub weight: Rat,
    pub payable_fraction: Rat,
    pub usd_per_kg: Rat,
    pub variable_usd_per_dry_t: Rat,
    pub fixed_usd: Rat,
    pub royalty_fraction: Rat,
    pub do_nothing_usd: Rat,
}

/// An explicit bridge from the admitted processing support to immediate cash.
/// No inferred price, sales timing, royalty regime or alternative payoff is supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationTerms {
    pub model_key: String,
    pub question: String,
    pub valuation_basis: String,
    pub action_id: String,
    pub action_label: String,
    pub boundary: String,
    pub circuit: String,
    pub material_domain: String,
    pub dry_basis: String,
    pub element: String,
    pub start_ns: i64,
    pub end_ns: i64,
    pub commercial_support: String,
    pub physical_to_commercial_basis: String,
    pub monetary_rounding: MonetaryRounding,
    pub scenarios: Vec<ScenarioTerms>,
}

fn put_rat(enc: &mut Encoder, value: Rat) {
    enc.put_fixed(&value.num().to_le_bytes());
    enc.put_fixed(&value.den().to_le_bytes());
}

impl ValuationTerms {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PVAL", 1);
        for text in [
            &self.model_key,
            &self.question,
            &self.valuation_basis,
            &self.action_id,
            &self.action_label,
            &self.boundary,
            &self.circuit,
            &self.material_domain,
            &self.dry_basis,
            &self.element,
            &self.commercial_support,
            &self.physical_to_commercial_basis,
            self.monetary_rounding.as_str(),
        ] {
            enc.put_str(text);
        }
        enc.put_i64(self.start_ns);
        enc.put_i64(self.end_ns);
        let mut rows: Vec<_> = self.scenarios.iter().collect();
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        enc.put_u64(rows.len() as u64);
        for row in rows {
            enc.put_str(&row.id);
            for value in [
                row.weight,
                row.payable_fraction,
                row.usd_per_kg,
                row.variable_usd_per_dry_t,
                row.fixed_usd,
                row.royalty_fraction,
                row.do_nothing_usd,
            ] {
                put_rat(&mut enc, value);
            }
        }
        format!("production-valuation-terms/v1:{}", sha256(&enc.finish()))
    }
}

/// All USD components are unrounded exact rationals. Only action payoffs cross
/// the final explicit Money boundary; deltas are rounded minus unrounded USD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationPoint {
    pub scenario_id: String,
    pub weight: Rat,
    pub represented_dry_tonnes: Rat,
    pub represented_recovered_kg: Rat,
    pub payable_kg: Rat,
    pub gross_revenue_usd: Rat,
    pub royalty_usd: Rat,
    pub variable_cost_usd: Rat,
    pub fixed_cost_usd: Rat,
    pub unrounded_margin_usd: Rat,
    pub margin: Money,
    pub margin_rounding_delta_usd: Rat,
    pub unrounded_do_nothing_usd: Rat,
    pub do_nothing: Money,
    pub do_nothing_rounding_delta_usd: Rat,
}

/// Immutable complete model, paired components and the existing decision evaluator's result.
#[derive(Debug, Clone, PartialEq)]
pub struct ValuationReceipt {
    production: ProductionReceipt,
    terms: Claim<ValuationTerms>,
    points: Vec<ValuationPoint>,
    decision: EvaluatedCard,
    digest: String,
    submitted_worksheet: Option<String>,
}

impl ValuationReceipt {
    pub fn production(&self) -> &ProductionReceipt {
        &self.production
    }
    pub fn terms(&self) -> &Claim<ValuationTerms> {
        &self.terms
    }
    pub fn points(&self) -> &[ValuationPoint] {
        &self.points
    }
    pub fn decision(&self) -> &EvaluatedCard {
        &self.decision
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("production valuation: {}", detail.into()))
}
fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| bad(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
}

/// Recover the exact finite binary64 value, with bounded rational storage.
/// Very small/large values outside i128 refuse instead of rounding or saturating.
fn represented(value: f64) -> Result<Rat> {
    let fail = || {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "physical value is outside the exact binary64-to-rational range",
        )
    };
    if !value.is_finite() || value < 0.0 {
        return Err(fail());
    }
    if value == 0.0 {
        return Ok(ZERO);
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = (bits & ((1_u64 << 52) - 1)) | if exponent == 0 { 0 } else { 1_u64 << 52 };
    let mut shift = if exponent == 0 {
        -1074
    } else {
        exponent - 1075
    };
    while shift < 0 && mantissa.is_multiple_of(2) {
        mantissa /= 2;
        shift += 1;
    }
    let power = 1_i128
        .checked_shl(shift.unsigned_abs())
        .filter(|p| *p > 0)
        .ok_or_else(fail)?;
    let mantissa = i128::from(mantissa);
    if shift >= 0 {
        exact(Rat::new(mantissa.checked_mul(power).ok_or_else(fail)?, 1))
    } else {
        exact(Rat::new(mantissa, power))
    }
}

/// Apply the declared USD settlement boundary to exact unrounded cash.
/// Returns the admitted amount and `rounded - unrounded` USD; neither evidence nor calibration changes.
pub fn round_declared_usd_cash(value: Rat, rule: MonetaryRounding) -> Result<(Money, Rat)> {
    money(value, rule)
}

fn money(value: Rat, rule: MonetaryRounding) -> Result<(Money, Rat)> {
    match rule {
        MonetaryRounding::NearestTiesEvenFourPlaces => {}
    }
    let scaled = exact(value.checked_mul(Rat::from_integer(Money::SCALE)))?;
    let (n, d) = (scaled.num(), scaled.den());
    let mut minor = n / d;
    let remainder = (n % d).abs();
    // Compare with half without doubling a remainder near the i128 limit.
    let cmp = remainder.cmp(&(d - remainder));
    if cmp.is_gt() || (cmp.is_eq() && minor % 2 != 0) {
        minor = minor.checked_add(n.signum()).ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "rounded money overflows",
            )
        })?;
    }
    let rounded = Money::from_minor(Currency::USD, minor);
    let delta = exact(exact(Rat::new(minor, Money::SCALE))?.checked_sub(value))?;
    Ok((rounded, delta))
}

fn check_claim<T>(claim: &Claim<T>) -> Result<()> {
    claim.check_id()?;
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale source Claim"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || claim.calibration_failed
    {
        return Err(bad(RefusalCode::NotObservable202, "unusable source Claim"));
    }
    if claim.support != SpatialSupport::PeriodAggregate {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "period-aggregate support required",
        ));
    }
    if claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "source hashes and locators required",
        ));
    }
    validate_color_payload(&claim.color()?).map_err(|_| {
        bad(
            RefusalCode::CalibrationRequired205,
            "invalid source colour metadata",
        )
    })?;
    Ok(())
}

struct Metadata {
    evidence: EvidenceClass,
    epistemic: EpistemicStatus,
    state: KnowledgeState,
    confidentiality: Confidentiality,
    uu: BTreeMap<String, UuTerm>,
    parents: BTreeSet<ClaimId>,
    anchors: BTreeSet<SourceAnchor>,
}

impl Metadata {
    fn new() -> Self {
        Self {
            evidence: EvidenceClass::ModelledUncalibrated,
            epistemic: EpistemicStatus::KnownUnknown(KuKind::Parametric),
            state: KnowledgeState::Estimated,
            confidentiality: Confidentiality::public(),
            uu: BTreeMap::new(),
            parents: BTreeSet::new(),
            anchors: BTreeSet::new(),
        }
    }
    fn include<T>(&mut self, claim: &Claim<T>) -> Result<()> {
        self.evidence = self.evidence.meet(claim.evidence);
        self.epistemic = self.epistemic.and(claim.epistemic);
        self.state = self.state.and(claim.state);
        self.confidentiality = self.confidentiality.join(&claim.confidentiality);
        self.parents.insert(claim.claim_id);
        self.anchors.extend(claim.anchors.iter().cloned());
        for term in &claim.uu_terms {
            if let Some(prior) = self.uu.insert(term.scenario_id.clone(), term.clone())
                && prior != *term
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting shared UU terms",
                ));
            }
        }
        Ok(())
    }
    fn claim<T>(&self, metric: &str, value: T, tag: &str, additivity: AdditivityClass) -> Claim<T> {
        Claim::new(
            metric,
            value,
            tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            self.evidence,
            self.epistemic,
            self.state,
            self.confidentiality.clone(),
            self.uu.values().cloned().collect(),
            self.parents.iter().copied().collect(),
            self.anchors.iter().cloned().collect(),
        )
    }
}

/// Join all commercial assumptions to the identical admitted physical scenarios,
/// calculate immediate cash margins, then call the existing exact decision engine.
/// Every material failure refuses the entire model; there is no partial population.
pub fn evaluate(
    production: &ProductionReceipt,
    terms: &Claim<ValuationTerms>,
) -> Result<ValuationReceipt> {
    check_claim(production.claim())?;
    check_claim(terms)?;
    if terms.metric_key != INPUT_METRIC || terms.value_tag() != terms.value.canonical_tag() {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "commercial terms differ from their Claim",
        ));
    }
    if terms.representation != Representation::Deterministic
        || terms.additivity != AdditivityClass::NonAdditive
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "explicit deterministic non-additive commercial terms required",
        ));
    }
    terms.verify_uu_slot_for_decision()?;
    if terms.uu_terms.is_empty()
        || terms.uu_terms.iter().any(|u| {
            !named(&u.scenario_id)
                || !named(&u.description)
                || !named(&u.trigger_indicator)
                || !u.downside_exposure_usd_m.is_finite()
                || u.downside_exposure_usd_m <= 0.0
        })
    {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "commercial terms require their own named UU exposure and trigger",
        ));
    }
    let t = &terms.value;
    if [
        &t.model_key,
        &t.question,
        &t.action_id,
        &t.action_label,
        &t.boundary,
        &t.circuit,
        &t.material_domain,
        &t.dry_basis,
        &t.element,
        &t.commercial_support,
        &t.physical_to_commercial_basis,
    ]
    .iter()
    .any(|s| !named(s))
        || t.action_id == DO_NOTHING
        || t.valuation_basis != VALUATION_BASIS
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "explicit named immediate-cash basis, support and distinct action required",
        ));
    }
    let physical = production.summary();
    if t.boundary != physical.grinding_scope.boundary
        || t.circuit != physical.grinding_scope.circuit
        || t.circuit != physical.recovery_scope.circuit
        || t.material_domain != physical.recovery_scope.material_domain
        || t.dry_basis != physical.recovery_scope.dry_basis
        || t.element != physical.recovery_scope.element
        || t.start_ns != physical.grinding_scope.interval.start_ns
        || t.end_ns != physical.grinding_scope.interval.end_ns
        || t.start_ns >= t.end_ns
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "commercial declaration does not match the complete physical support",
        ));
    }
    if t.scenarios.is_empty()
        || t.scenarios.len() > card::MAX_STATES
        || t.scenarios.len() != production.points().len()
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "commercial and production populations must match completely",
        ));
    }
    let mut by_id = BTreeMap::new();
    let mut mass = ZERO;
    for row in &t.scenarios {
        if !named(&row.id) || by_id.insert(row.id.as_str(), row).is_some() {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "empty or duplicate scenario id",
            ));
        }
        if row.weight.signum() <= 0
            || row.payable_fraction.signum() < 0
            || exact(row.payable_fraction.checked_cmp(ONE))?.is_gt()
            || row.royalty_fraction.signum() < 0
            || exact(row.royalty_fraction.checked_cmp(ONE))?.is_gt()
            || [row.usd_per_kg, row.variable_usd_per_dry_t, row.fixed_usd]
                .iter()
                .any(|v| v.signum() < 0)
        {
            return Err(bad(
                RefusalCode::OutsidePhysicalDomain109,
                "positive weights, fractions in [0,1] and nonnegative prices/costs required",
            ));
        }
        mass = exact(mass.checked_add(row.weight))?;
    }
    if mass != ONE {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "scenario weights must sum exactly to one",
        ));
    }
    let mut points = Vec::with_capacity(t.scenarios.len());
    for physical in production.points() {
        let row = by_id.remove(physical.scenario_id.as_str()).ok_or_else(|| {
            bad(
                RefusalCode::BasisMismatch105,
                "commercial state is missing or foreign",
            )
        })?;
        if row.weight != physical.weight {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "physical and commercial probabilities differ",
            ));
        }
        let tonnes = represented(physical.dry_tonnes.value())?;
        let recovered = represented(physical.recovered_metal.value())?;
        let payable = exact(recovered.checked_mul(row.payable_fraction))?;
        let revenue = exact(payable.checked_mul(row.usd_per_kg))?;
        let royalty = exact(revenue.checked_mul(row.royalty_fraction))?;
        let variable = exact(tonnes.checked_mul(row.variable_usd_per_dry_t))?;
        let net = exact(
            exact(exact(revenue.checked_sub(royalty))?.checked_sub(variable))?
                .checked_sub(row.fixed_usd),
        )?;
        let (margin, margin_delta) = money(net, t.monetary_rounding)?;
        let (do_nothing, do_nothing_delta) = money(row.do_nothing_usd, t.monetary_rounding)?;
        points.push(ValuationPoint {
            scenario_id: row.id.clone(),
            weight: row.weight,
            represented_dry_tonnes: tonnes,
            represented_recovered_kg: recovered,
            payable_kg: payable,
            gross_revenue_usd: revenue,
            royalty_usd: royalty,
            variable_cost_usd: variable,
            fixed_cost_usd: row.fixed_usd,
            unrounded_margin_usd: net,
            margin,
            margin_rounding_delta_usd: margin_delta,
            unrounded_do_nothing_usd: row.do_nothing_usd,
            do_nothing,
            do_nothing_rounding_delta_usd: do_nothing_delta,
        });
    }
    if !by_id.is_empty() {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "foreign commercial states remain",
        ));
    }
    points.sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
    let mut meta = Metadata::new();
    meta.include(production.claim())?;
    meta.include(terms)?;
    let weights = WeightSet {
        model_key: t.model_key.clone(),
        question: t.question.clone(),
        states: points
            .iter()
            .map(|p| (p.scenario_id.clone(), p.weight))
            .collect(),
    };
    let weights_tag = weights.canonical_tag();
    let weights = meta.claim(
        "production.valuation.weights/1",
        weights,
        &weights_tag,
        AdditivityClass::NonAdditive,
    );
    let mut actions = Vec::new();
    for (id, label, role) in [
        (
            t.action_id.as_str(),
            t.action_label.as_str(),
            ActionRole::Action,
        ),
        (
            DO_NOTHING,
            "Do nothing (declared immediate cash)",
            ActionRole::DoNothing,
        ),
    ] {
        let action = ActionPayoffs {
            model_key: t.model_key.clone(),
            action_id: id.into(),
            label: label.into(),
            role,
            currency: Currency::USD,
            valuation_basis: t.valuation_basis.clone(),
            values: points
                .iter()
                .map(|p| {
                    (
                        p.scenario_id.clone(),
                        if role == ActionRole::Action {
                            p.margin
                        } else {
                            p.do_nothing
                        },
                    )
                })
                .collect(),
        };
        let tag = action.canonical_tag();
        actions.push(meta.claim(
            "production.valuation.action/1",
            action,
            &tag,
            AdditivityClass::ExtensiveAdditive,
        ));
    }
    let decision = card::evaluate(&DecisionInputs {
        weights,
        actions,
        information_options: Vec::new(),
        robustness: None,
        downside_limit: None,
    })?;
    let mut enc = Encoder::new_frame(*b"PVRC", 1);
    enc.put_str(production.digest());
    // CLM1 ids omit UU, support, representation and supersession metadata.
    // Bind the full admitted snapshots as well as the existing decision receipt.
    enc.put_str(&render::claim(production.claim()));
    enc.put_str(&render::claim(production.inputs()));
    enc.put_str(&render::claim(terms));
    enc.put_str(&decision.digest());
    enc.put_u64(points.len() as u64);
    for p in &points {
        enc.put_str(&p.scenario_id);
        for value in [
            p.weight,
            p.represented_dry_tonnes,
            p.represented_recovered_kg,
            p.payable_kg,
            p.gross_revenue_usd,
            p.royalty_usd,
            p.variable_cost_usd,
            p.fixed_cost_usd,
            p.unrounded_margin_usd,
            p.margin_rounding_delta_usd,
            p.unrounded_do_nothing_usd,
            p.do_nothing_rounding_delta_usd,
        ] {
            put_rat(&mut enc, value);
        }
        enc.put_fixed(&p.margin.minor.to_le_bytes());
        enc.put_fixed(&p.do_nothing.minor.to_le_bytes());
    }
    let digest = sha256(&enc.finish()).to_hex();
    Ok(ValuationReceipt {
        production: production.clone(),
        terms: terms.clone(),
        points,
        decision,
        digest,
        submitted_worksheet: None,
    })
}
