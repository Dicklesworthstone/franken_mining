//! DESIGN (BD-225/229): declared-period investment values from paired operating cash receipts.
//! Determinism: DeterministicPerIsa (the inherited physical calculations); all new arithmetic is exact.

use super::*;
use fmine_decide::card::downside::DownsideLimit;

mod render;
#[cfg(test)]
mod tests;
pub mod worksheet;

pub const INPUT_METRIC: &str = "production.investment.declaration/1";
pub const VALUATION_BASIS: &str = "declared_periods_discounted_incremental_cash";
pub const CASHFLOW_BASIS: &str = "incremental_operating_cash_excludes_initial_capex";
pub const MAX_ALTERNATIVES: usize = 8;
pub const MAX_PERIODS: usize = 16;
pub const MAX_STATE_PERIODS: usize = 65_536;
pub const NO_CLAIM: &str = "Conditional investment comparison over the declared nonoverlapping operating periods and complete shared scenarios. Each operating margin is compared with its same-period do-nothing cash, discounted by the explicit factor, and charged the declared initial investment once at valuation time. Do-nothing has zero incremental value. Only the declared cash window is valued; no omitted-period cash, terminal value, tax, ownership, financing, annual discount convention, project lifetime or NAV is inferred. Scenario meaning across time and alternatives, discount factors and initial investments are source assumptions. Investments are mutually exclusive alternatives, not a combined portfolio. Inputs remain uncalibrated; no investment suitability, physical feasibility, sign-off or execution authorization is supplied.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestmentPeriod {
    pub id: String,
    pub start_ns: i64,
    pub end_ns: i64,
    /// Present-value multiplier at valuation_time_ns; explicitly supplied, never inferred from a rate.
    pub discount_factor: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestmentAlternative {
    pub id: String,
    pub label: String,
    /// A nonnegative USD payment at valuation_time_ns, excluded from all supplied operating margins.
    pub initial_capex_usd: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestmentSpec {
    pub model_key: String,
    pub question: String,
    pub shared_scenario_basis: String,
    pub discount_basis: String,
    pub cashflow_basis: String,
    pub currency: Currency,
    pub valuation_time_ns: i64,
    pub boundary: String,
    pub circuit: String,
    pub material_domain: String,
    pub dry_basis: String,
    pub element: String,
    pub commercial_support: String,
    pub monetary_rounding: MonetaryRounding,
    /// Screen final discounted incremental value after capex and the declared Money rounding.
    pub downside_limit: Option<DownsideLimit>,
    pub periods: Vec<InvestmentPeriod>,
    pub alternatives: Vec<InvestmentAlternative>,
}

impl InvestmentSpec {
    pub fn canonical_tag(&self) -> String {
        format!(
            "production-investment/v1:{}",
            sha256(render::spec(self).as_bytes())
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeriodValuation {
    pub alternative_id: String,
    pub period_id: String,
    pub valuation: ValuationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestmentCashFlow {
    pub alternative_id: String,
    pub period_id: String,
    pub scenario_id: String,
    pub weight: Rat,
    pub operating_cash_usd: Rat,
    pub do_nothing_cash_usd: Rat,
    pub incremental_cash_usd: Rat,
    pub discounted_incremental_cash_usd: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestmentOutcome {
    pub alternative_id: String,
    pub scenario_id: String,
    pub weight: Rat,
    pub discounted_operating_delta_usd: Rat,
    pub unrounded_value_usd: Rat,
    pub value: Money,
    pub rounding_delta_usd: Rat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvestmentReceipt {
    declaration: Claim<InvestmentSpec>,
    bundles: Vec<PeriodValuation>,
    cashflows: Vec<InvestmentCashFlow>,
    outcomes: Vec<InvestmentOutcome>,
    decision: EvaluatedCard,
    digest: String,
    submitted_manifest: Option<String>,
}

impl InvestmentReceipt {
    pub fn declaration(&self) -> &Claim<InvestmentSpec> {
        &self.declaration
    }
    pub fn bundles(&self) -> &[PeriodValuation] {
        &self.bundles
    }
    pub fn cashflows(&self) -> &[InvestmentCashFlow] {
        &self.cashflows
    }
    pub fn outcomes(&self) -> &[InvestmentOutcome] {
        &self.outcomes
    }
    pub fn decision(&self) -> &EvaluatedCard {
        &self.decision
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn invalid(message: &str) -> Refusal {
    bad(RefusalCode::BasisMismatch105, message)
}

fn validate_spec(spec: &InvestmentSpec) -> Result<()> {
    if let Some(limit) = &spec.downside_limit {
        if limit.model_key != spec.model_key
            || limit.valuation_basis != VALUATION_BASIS
            || limit.floor.ccy != spec.currency
        {
            return Err(invalid(
                "investment downside limit must share the model, currency and discounted incremental cash basis",
            ));
        }
        if limit.max_breach_probability.num() < 0
            || limit.max_breach_probability.num() > limit.max_breach_probability.den()
        {
            return Err(bad(
                RefusalCode::OutsidePhysicalDomain109,
                "maximum investment downside breach probability must lie in [0,1]",
            ));
        }
        if limit.policy_optimization.is_some() {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                "investment comparisons screen complete investment choices; no information policies are declared",
            ));
        }
    }
    if !(1..=MAX_ALTERNATIVES).contains(&spec.alternatives.len())
        || !(1..=MAX_PERIODS).contains(&spec.periods.len())
        || spec.currency != Currency::USD
        || spec.cashflow_basis != CASHFLOW_BASIS
        || [
            &spec.model_key,
            &spec.question,
            &spec.shared_scenario_basis,
            &spec.discount_basis,
            &spec.boundary,
            &spec.circuit,
            &spec.material_domain,
            &spec.dry_basis,
            &spec.element,
            &spec.commercial_support,
        ]
        .iter()
        .any(|s| !named(s))
    {
        return Err(invalid(
            "one to eight investments, one to sixteen periods, USD, explicit incremental cash basis and complete named support required",
        ));
    }
    let mut ids = BTreeSet::new();
    for a in &spec.alternatives {
        if !named(&a.id)
            || !named(&a.label)
            || a.id == DO_NOTHING
            || !ids.insert(&a.id)
            || a.initial_capex_usd.signum() < 0
        {
            return Err(invalid(
                "unique investment ids separate from do-nothing, labels and nonnegative initial capex required",
            ));
        }
    }
    let mut ids = BTreeSet::new();
    let mut periods = spec.periods.iter().collect::<Vec<_>>();
    periods.sort_by_key(|p| (p.start_ns, p.end_ns, &p.id));
    let mut previous_end = spec.valuation_time_ns;
    for p in periods {
        if !named(&p.id)
            || !ids.insert(&p.id)
            || p.start_ns < previous_end
            || p.start_ns >= p.end_ns
            || p.discount_factor.signum() <= 0
        {
            return Err(invalid(
                "unique positive-duration periods must not overlap or precede valuation time; positive explicit discount factors required",
            ));
        }
        previous_end = p.end_ns;
    }
    Ok(())
}

/// Derive one incremental cash distribution per mutually exclusive investment.
/// Unrounded same-state operating cash is aggregated before the sole new Money rounding boundary.
pub fn evaluate(
    declaration: &Claim<InvestmentSpec>,
    bundles: &[PeriodValuation],
) -> Result<InvestmentReceipt> {
    let spec = &declaration.value;
    validate_spec(spec)?;
    check_claim(declaration)?;
    if declaration.metric_key != INPUT_METRIC
        || declaration.value_tag() != spec.canonical_tag()
        || declaration.representation != Representation::Deterministic
        || declaration.additivity != AdditivityClass::NonAdditive
    {
        return Err(invalid(
            "investment declaration identity or representation does not match",
        ));
    }
    declaration.verify_uu_slot_for_decision()?;
    if declaration.uu_terms.is_empty()
        || declaration.uu_terms.iter().any(|u| {
            !named(&u.scenario_id)
                || !named(&u.description)
                || !named(&u.trigger_indicator)
                || !u.downside_exposure_usd_m.is_finite()
                || u.downside_exposure_usd_m <= 0.0
        })
    {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "investment assumptions require a named positive UU exposure and trigger",
        ));
    }
    if bundles.len() != spec.alternatives.len() * spec.periods.len() {
        return Err(invalid(
            "every investment requires exactly one valuation bundle for every period",
        ));
    }
    let alternatives = spec
        .alternatives
        .iter()
        .map(|a| (a.id.as_str(), a))
        .collect::<BTreeMap<_, _>>();
    let periods = spec
        .periods
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = BTreeMap::new();
    let mut all_meta = Metadata::new();
    all_meta.include(declaration)?;
    let mut work = 0_usize;
    for b in bundles {
        let Some(period) = periods.get(b.period_id.as_str()) else {
            return Err(invalid("foreign period bundle"));
        };
        let t = &b.valuation.terms.value;
        if !alternatives.contains_key(b.alternative_id.as_str())
            || b.alternative_id != t.action_id
            || ordered
                .insert((b.alternative_id.as_str(), b.period_id.as_str()), b)
                .is_some()
        {
            return Err(invalid(
                "foreign, duplicate or mismatched investment bundle",
            ));
        }
        if t.boundary != spec.boundary
            || t.circuit != spec.circuit
            || t.material_domain != spec.material_domain
            || t.dry_basis != spec.dry_basis
            || t.element != spec.element
            || t.commercial_support != spec.commercial_support
            || t.start_ns != period.start_ns
            || t.end_ns != period.end_ns
            || t.valuation_basis != super::VALUATION_BASIS
            || t.monetary_rounding != spec.monetary_rounding
        {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                "investment bundle does not match its complete physical/commercial period support",
            ));
        }
        work = work
            .checked_add(b.valuation.points.len())
            .ok_or_else(|| invalid("investment work count overflow"))?;
        if work > MAX_STATE_PERIODS {
            return Err(invalid("investment exceeds 65536 bundle/state rows"));
        }
        check_claim(b.valuation.decision.claim())?;
        all_meta.include(b.valuation.decision.claim())?;
    }
    let first = ordered
        .values()
        .next()
        .ok_or_else(|| invalid("no investment valuations"))?;
    let population = &first.valuation.points;
    let mut baselines = BTreeMap::new();
    for b in ordered.values() {
        if b.valuation.points.len() != population.len()
            || b.valuation
                .points
                .iter()
                .zip(population)
                .any(|(a, b)| a.scenario_id != b.scenario_id || a.weight != b.weight)
            || b.valuation.terms.value.physical_to_commercial_basis
                != first.valuation.terms.value.physical_to_commercial_basis
        {
            return Err(invalid(
                "all investment periods require the same complete states, exact weights and commercial bridge",
            ));
        }
        for point in &b.valuation.points {
            let key = (&b.period_id, &point.scenario_id);
            if baselines
                .insert(key, point.unrounded_do_nothing_usd)
                .is_some_and(|v| v != point.unrounded_do_nothing_usd)
            {
                return Err(invalid(
                    "same-period do-nothing cash must agree before rounding across every investment",
                ));
            }
        }
    }
    let weights = WeightSet {
        model_key: spec.model_key.clone(),
        question: spec.question.clone(),
        states: population
            .iter()
            .map(|p| (p.scenario_id.clone(), p.weight))
            .collect(),
    };
    let tag = weights.canonical_tag();
    let weights = all_meta.claim(
        "production.investment.weights/1",
        weights,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let (mut cashflows, mut outcomes, mut actions) =
        (Vec::with_capacity(work), Vec::new(), Vec::new());
    for alternative in alternatives.values() {
        let mut meta = Metadata::new();
        meta.include(declaration)?;
        let mut totals = vec![ZERO; population.len()];
        for period in periods.values() {
            let bundle = ordered
                .get(&(alternative.id.as_str(), period.id.as_str()))
                .ok_or_else(|| invalid("missing investment-period valuation"))?;
            meta.include(bundle.valuation.decision.claim())?;
            for (total, p) in totals.iter_mut().zip(&bundle.valuation.points) {
                let increment = exact(
                    p.unrounded_margin_usd
                        .checked_sub(p.unrounded_do_nothing_usd),
                )?;
                let discounted = exact(increment.checked_mul(period.discount_factor))?;
                *total = exact(total.checked_add(discounted))?;
                cashflows.push(InvestmentCashFlow {
                    alternative_id: alternative.id.clone(),
                    period_id: period.id.clone(),
                    scenario_id: p.scenario_id.clone(),
                    weight: p.weight,
                    operating_cash_usd: p.unrounded_margin_usd,
                    do_nothing_cash_usd: p.unrounded_do_nothing_usd,
                    incremental_cash_usd: increment,
                    discounted_incremental_cash_usd: discounted,
                });
            }
        }
        let mut values = Vec::with_capacity(population.len());
        for (total, p) in totals.into_iter().zip(population) {
            let unrounded = exact(total.checked_sub(alternative.initial_capex_usd))?;
            let (value, delta) = money(unrounded, spec.monetary_rounding)?;
            values.push((p.scenario_id.clone(), value));
            outcomes.push(InvestmentOutcome {
                alternative_id: alternative.id.clone(),
                scenario_id: p.scenario_id.clone(),
                weight: p.weight,
                discounted_operating_delta_usd: total,
                unrounded_value_usd: unrounded,
                value,
                rounding_delta_usd: delta,
            });
        }
        let action = ActionPayoffs {
            model_key: spec.model_key.clone(),
            action_id: alternative.id.clone(),
            label: alternative.label.clone(),
            role: ActionRole::Action,
            currency: Currency::USD,
            valuation_basis: VALUATION_BASIS.into(),
            values,
        };
        let tag = action.canonical_tag();
        actions.push(meta.claim(
            "production.investment.action/1",
            action,
            &tag,
            AdditivityClass::ExtensiveAdditive,
        ));
    }
    let hold = ActionPayoffs {
        model_key: spec.model_key.clone(),
        action_id: DO_NOTHING.into(),
        label: "Do nothing (zero incremental value relative to declared baseline)".into(),
        role: ActionRole::DoNothing,
        currency: Currency::USD,
        valuation_basis: VALUATION_BASIS.into(),
        values: population
            .iter()
            .map(|p| (p.scenario_id.clone(), Money::from_minor(Currency::USD, 0)))
            .collect(),
    };
    let tag = hold.canonical_tag();
    actions.push(all_meta.claim(
        "production.investment.action/1",
        hold,
        &tag,
        AdditivityClass::ExtensiveAdditive,
    ));
    let downside_limit = spec.downside_limit.as_ref().map(|limit| {
        all_meta.claim(
            "production.investment.downside_limit/1",
            limit.clone(),
            &limit.canonical_tag(),
            AdditivityClass::NonAdditive,
        )
    });
    let decision = card::evaluate(&DecisionInputs {
        weights,
        actions,
        information_options: Vec::new(),
        robustness: None,
        downside_limit,
    })?;
    let mut enc = Encoder::new_frame(*b"PVIR", 1);
    enc.put_str(&super::render::claim(declaration));
    for ((alternative, period), b) in &ordered {
        enc.put_str(alternative);
        enc.put_str(period);
        enc.put_str(b.valuation.digest());
    }
    enc.put_str(&decision.digest());
    // Bind the exact pre-rounding calculation as well as its rounded decision.
    enc.put_u64(cashflows.len() as u64);
    for p in &cashflows {
        enc.put_str(&p.alternative_id);
        enc.put_str(&p.period_id);
        enc.put_str(&p.scenario_id);
        for value in [
            p.weight,
            p.operating_cash_usd,
            p.do_nothing_cash_usd,
            p.incremental_cash_usd,
            p.discounted_incremental_cash_usd,
        ] {
            put_rat(&mut enc, value);
        }
    }
    enc.put_u64(outcomes.len() as u64);
    for p in &outcomes {
        enc.put_str(&p.alternative_id);
        enc.put_str(&p.scenario_id);
        for value in [
            p.weight,
            p.discounted_operating_delta_usd,
            p.unrounded_value_usd,
            p.rounding_delta_usd,
        ] {
            put_rat(&mut enc, value);
        }
        enc.put_fixed(&p.value.minor.to_le_bytes());
    }
    Ok(InvestmentReceipt {
        declaration: declaration.clone(),
        bundles: ordered.into_values().cloned().collect(),
        cashflows,
        outcomes,
        decision,
        digest: sha256(&enc.finish()).to_hex(),
        submitted_manifest: None,
    })
}
