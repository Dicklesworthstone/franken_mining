//! DESIGN (BD-229, §23.6): exact economic sacrifice of declared KPI constraints on static portfolios.
//! Determinism: Exact. Whole-bundle components are summed before ratios or scenario aggregation.

use crate::*;

mod render;
#[cfg(test)]
mod tests;

pub const MAX_CONSTRAINTS: usize = 8;
pub const MAX_COMPONENT_WORK: usize = 1_048_576;
pub const NO_CLAIM: &str = "Static KPI-constrained selection conditional on the declared reconciled additive metric definitions, component matrices and original shared-scenario capital model. All original structural and nominal risk constraints remain binding. Ratios use whole-bundle numerators and positive denominators; the explicit aggregation is either a ratio of expected totals or a limit in every original state. KPI thresholds are inclusive. Opportunity costs and threshold frontiers are exact finite portfolio differences in the primary USD or USD/share objective, not LP shadow prices, causal effects or derivatives. No safety constraint is priced, issuer approval inferred, metric definition reconciled automatically, calibrated probability, adaptive policy, new bootstrap/DRO assessment or execution authority supplied.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric { GoldAisc, CopperAisc, Roce, Geo }
impl Metric {
    pub fn as_str(self) -> &'static str {
        match self { Self::GoldAisc => "gold_aisc", Self::CopperAisc => "copper_aisc", Self::Roce => "roce", Self::Geo => "geo" }
    }
    pub fn unit(self) -> &'static str {
        match self { Self::GoldAisc => "USD/oz_t_au", Self::CopperAisc => "USD/lb_cu", Self::Roce => "USD/USD", Self::Geo => "GEO_oz" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation { RatioOfExpectedTotals, EveryState }
impl Aggregation {
    pub fn as_str(self) -> &'static str {
        match self { Self::RatioOfExpectedTotals => "ratio_of_expected_totals", Self::EveryState => "every_state" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { Upper, Lower }
impl Direction {
    pub fn as_str(self) -> &'static str {
        match self { Self::Upper => "upper", Self::Lower => "lower" }
    }
    fn passes(self, value: Rat, limit: Rat) -> bool {
        match self { Self::Upper => !exact_compare(value, limit).is_gt(), Self::Lower => !exact_compare(value, limit).is_lt() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline { pub state_id: String, pub numerator: Rat, pub denominator: Rat }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Increment { pub candidate_id: String, pub state_id: String, pub numerator: Rat, pub denominator: Rat }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub id: String,
    pub metric: Metric,
    /// Declares already-reconciled scope, ownership, cost/production definitions and adjustments.
    pub basis: String,
    /// Inclusive YYYY-MM-DD/YYYY-MM-DD; no reporting calendar is inferred.
    pub period: String,
    pub aggregation: Aggregation,
    pub direction: Direction,
    pub limit: Rat,
    pub baseline: Vec<Baseline>,
    pub increments: Vec<Increment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Design { pub model_key: String, pub constraints: Vec<Constraint> }
impl Design {
    pub fn canonical_tag(&self) -> String {
        crate::render::value_tag("capital-kpi-design/v1", &render::design_json(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateValue {
    pub state_id: String,
    pub numerator: Rat,
    pub denominator: Rat,
    /// Absent only for a nonpositive denominator in an already structurally/risk-excluded bundle.
    pub ratio: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub constraint_id: String,
    pub states: Vec<StateValue>,
    /// Ratio of weighted totals, or the worst state value in the declared inequality direction.
    pub threshold_value: Option<Rat>,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub bundle_id: String,
    pub base_eligible: bool,
    pub objective_mean: Option<Rat>,
    pub metrics: Vec<Assessment>,
    pub kpi_rejections: Vec<String>,
    pub feasible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontierPoint {
    pub limit: Rat,
    pub at_declared_limit: bool,
    pub feasible_count: usize,
    pub winners: Vec<String>,
    pub objective_mean: Option<Rat>,
    /// Relative to the best original structurally and nominally risk-feasible portfolio.
    pub opportunity_cost: Option<Rat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontier {
    pub constraint_id: String,
    /// Ordered from tighter to looser limits; every other declared KPI limit stays fixed.
    pub points: Vec<FrontierPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub model_key: String,
    pub objective_unit: String,
    pub baseline_winners: Vec<String>,
    pub baseline_objective: Option<Rat>,
    pub constrained_winners: Vec<String>,
    pub constrained_objective: Option<Rat>,
    pub opportunity_cost: Option<Rat>,
    pub choices: Vec<Choice>,
    pub frontiers: Vec<Frontier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedKpiProgram {
    program: EvaluatedProgram,
    design: Claim<Design>,
    claim: Claim<Summary>,
    digest: String,
}
impl EvaluatedKpiProgram {
    pub fn static_program(&self) -> &EvaluatedProgram { &self.program }
    pub fn design(&self) -> &Claim<Design> { &self.design }
    pub fn summary(&self) -> &Summary { &self.claim.value }
    pub fn claim(&self) -> &Claim<Summary> { &self.claim }
    pub fn digest(&self) -> &str { &self.digest }
}

fn invalid(why: impl Into<String>) -> Refusal { bad(RefusalCode::NoClaimInDomain201, why) }

fn validate(program: &EvaluatedProgram, claim: &Claim<Design>) -> Result<()> {
    let inputs = program.inputs();
    if inputs.robustness.is_some() || inputs.selection_stability.is_some() {
        return Err(invalid("KPI comparison does not reoptimize Wasserstein or bootstrap constraints; supply a static program without those optional diagnostics"));
    }
    let design = &claim.value;
    let state_ids: BTreeSet<_> = inputs.weights.value.states.iter().map(|s| s.0.as_str()).collect();
    let candidate_ids: BTreeSet<_> = inputs.candidates.iter().map(|c| c.value.candidate_id.as_str()).collect();
    let work = program.summary().choices.len()
        .checked_mul(design.constraints.len())
        .and_then(|n| n.checked_mul(state_ids.len()))
        .and_then(|n| n.checked_mul(candidate_ids.len() + 1));
    if !(1..=MAX_CONSTRAINTS).contains(&design.constraints.len())
        || work.is_none_or(|n| n > MAX_COMPONENT_WORK)
        || design.constraints.iter().any(|c| c.baseline.len() != state_ids.len()
            || c.increments.len() != candidate_ids.len() * state_ids.len())
    {
        return Err(invalid("KPI comparison requires 1..=8 constraints, complete baseline/candidate-state components and at most 1048576 component additions"));
    }
    admit(claim, &design.canonical_tag(), AdditivityClass::NonAdditive)?;
    if design.model_key != inputs.policy.value.model_key {
        return Err(bad(RefusalCode::BasisMismatch105, "KPI design names another capital model"));
    }
    let mut names = BTreeSet::new();
    for constraint in &design.constraints {
        if !key(&constraint.id) || !names.insert(&constraint.id) || !named(&constraint.basis) {
            return Err(invalid("KPI constraints need unique bounded identifiers and an explicit reconciled metric basis"));
        }
        if !constraint.period.split_once('/').is_some_and(|(start, end)|
            fmine_registry::sources::is_date(start) && fmine_registry::sources::is_date(end) && start <= end)
        {
            return Err(bad(RefusalCode::UndeclaredUnitBasisOrTz107, "KPI period must be an ordered inclusive YYYY-MM-DD/YYYY-MM-DD range"));
        }
        let mut states = BTreeSet::new();
        for row in &constraint.baseline {
            if !state_ids.contains(row.state_id.as_str()) || !states.insert(row.state_id.as_str()) {
                return Err(bad(RefusalCode::BasisMismatch105, "KPI baseline must name every original state exactly once"));
            }
            if constraint.metric == Metric::Geo && row.denominator != ONE {
                return Err(bad(RefusalCode::UndeclaredUnitBasisOrTz107, "GEO baseline denominator must be exactly one; declare the fixed-price conversion in its basis"));
            }
        }
        let mut pairs = BTreeSet::new();
        for row in &constraint.increments {
            if !state_ids.contains(row.state_id.as_str()) || !candidate_ids.contains(row.candidate_id.as_str())
                || !pairs.insert((row.candidate_id.as_str(), row.state_id.as_str()))
            {
                return Err(bad(RefusalCode::BasisMismatch105, "KPI increments must name every original candidate/state pair exactly once"));
            }
            if constraint.metric == Metric::Geo && row.denominator != ZERO {
                return Err(bad(RefusalCode::UndeclaredUnitBasisOrTz107, "GEO denominator increments must be explicitly zero"));
            }
        }
    }
    Ok(())
}

fn assess(
    constraint: &Constraint,
    choice: &ProgramChoice,
    weights: &[(String, Rat)],
    base_eligible: bool,
) -> Result<Assessment> {
    let baseline: BTreeMap<_, _> = constraint.baseline.iter().map(|r| (r.state_id.as_str(), r)).collect();
    let increments: BTreeMap<_, _> = constraint.increments.iter()
        .map(|r| ((r.candidate_id.as_str(), r.state_id.as_str()), r)).collect();
    let missing = || invalid("KPI calculation lost an admitted state or candidate component");
    let mut states = Vec::with_capacity(weights.len());
    let mut weighted_numerator = ZERO;
    let mut weighted_denominator = ZERO;
    let mut valid = true;
    let mut worst = None;
    for (state, weight) in weights {
        let b = baseline.get(state.as_str()).ok_or_else(missing)?;
        let (mut numerator, mut denominator) = (b.numerator, b.denominator);
        for candidate in &choice.selected {
            let delta = increments.get(&(candidate.as_str(), state.as_str())).ok_or_else(missing)?;
            numerator = arithmetic(numerator.checked_add(delta.numerator))?;
            denominator = arithmetic(denominator.checked_add(delta.denominator))?;
        }
        let ratio = if denominator.signum() <= 0 {
            if base_eligible {
                return Err(bad(RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("KPI {} has a nonpositive denominator for eligible bundle {} in state {state}; supply a positive denominator on every considered path", constraint.id, choice.bundle_id)));
            }
            valid = false;
            None
        } else {
            Some(arithmetic(numerator.checked_div(denominator))?)
        };
        weighted_numerator = arithmetic(weighted_numerator.checked_add(arithmetic(numerator.checked_mul(*weight))?))?;
        weighted_denominator = arithmetic(weighted_denominator.checked_add(arithmetic(denominator.checked_mul(*weight))?))?;
        if let Some(value) = ratio {
            worst = Some(worst.map_or(value, |prior| {
                if constraint.direction.passes(value, prior) { prior } else { value }
            }));
        }
        states.push(StateValue { state_id: state.clone(), numerator, denominator, ratio });
    }
    let threshold_value = if !valid {
        None
    } else {
        match constraint.aggregation {
            Aggregation::RatioOfExpectedTotals => Some(arithmetic(weighted_numerator.checked_div(weighted_denominator))?),
            Aggregation::EveryState => worst,
        }
    };
    Ok(Assessment {
        constraint_id: constraint.id.clone(), states,
        passes: threshold_value.is_some_and(|value| constraint.direction.passes(value, constraint.limit)),
        threshold_value,
    })
}

fn best<'a>(rows: impl Iterator<Item = &'a Choice>) -> (usize, Vec<String>, Option<Rat>) {
    let mut count = 0;
    let mut winners = Vec::new();
    let mut maximum = None;
    for row in rows {
        let Some(value) = row.objective_mean else { continue };
        count += 1;
        match maximum.map(|prior| exact_compare(value, prior)) {
            None | Some(std::cmp::Ordering::Greater) => {
                maximum = Some(value); winners.clear(); winners.push(row.bundle_id.clone());
            }
            Some(std::cmp::Ordering::Equal) => winners.push(row.bundle_id.clone()),
            Some(std::cmp::Ordering::Less) => {}
        }
    }
    (count, winners, maximum)
}

fn sacrifice(base: Option<Rat>, restricted: Option<Rat>) -> Result<Option<Rat>> {
    match (base, restricted) {
        (Some(base), Some(restricted)) => {
            let loss = arithmetic(base.checked_sub(restricted))?;
            if loss.signum() < 0 {
                return Err(bad(RefusalCode::NumericalCertificateFailed207, "a KPI restriction increased the maximum objective"));
            }
            Ok(Some(loss))
        }
        _ => Ok(None),
    }
}

/// Screen every nominally eligible static bundle and enumerate exact one-threshold frontiers.
/// Other KPI limits, original cash/structural rules and all nominal risk limits remain fixed.
pub fn evaluate(program: &EvaluatedProgram, design: &Claim<Design>) -> Result<EvaluatedKpiProgram> {
    validate(program, design)?;
    let mut design = design.clone();
    design.value.constraints.sort_by(|a, b| a.id.cmp(&b.id));
    for constraint in &mut design.value.constraints {
        constraint.baseline.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        constraint.increments.sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    }
    let mut choices = Vec::with_capacity(program.summary().choices.len());
    for choice in &program.summary().choices {
        let base_eligible = choice.structural_rejections.is_empty() && !choice.risk_rejected;
        let objective_mean = if program.inputs().equity.is_some() {
            choice.per_share_statistics.as_ref().map(|s| s.mean.major)
        } else {
            choice.statistics.as_ref().map(|s| s.mean.major)
        };
        if base_eligible && objective_mean.is_none() {
            return Err(bad(RefusalCode::NumericalCertificateFailed207, "eligible capital bundle has no objective mean"));
        }
        let metrics = design.value.constraints.iter()
            .map(|c| assess(c, choice, &program.inputs().weights.value.states, base_eligible))
            .collect::<Result<Vec<_>>>()?;
        let kpi_rejections: Vec<_> = metrics.iter().filter(|m| !m.passes).map(|m| m.constraint_id.clone()).collect();
        choices.push(Choice {
            bundle_id: choice.bundle_id.clone(), base_eligible, objective_mean, metrics,
            feasible: base_eligible && kpi_rejections.is_empty(), kpi_rejections,
        });
    }
    choices.sort_by(|a, b| a.bundle_id.cmp(&b.bundle_id));
    let (_, baseline_winners, baseline_objective) = best(choices.iter().filter(|c| c.base_eligible));
    let (_, constrained_winners, constrained_objective) = best(choices.iter().filter(|c| c.feasible));
    let mut frontiers = Vec::new();
    for (index, constraint) in design.value.constraints.iter().enumerate() {
        let eligible: Vec<_> = choices.iter().filter(|c| c.base_eligible
            && c.metrics.iter().enumerate().all(|(i, m)| i == index || m.passes)).collect();
        let mut limits: Vec<_> = eligible.iter().filter_map(|c| c.metrics[index].threshold_value).collect();
        limits.push(constraint.limit);
        limits.sort_by(|a, b| exact_compare(*a, *b));
        limits.dedup();
        if constraint.direction == Direction::Lower { limits.reverse(); }
        let mut points = Vec::with_capacity(limits.len());
        for limit in limits {
            let (feasible_count, winners, objective_mean) = best(eligible.iter().copied().filter(|c|
                c.metrics[index].threshold_value.is_some_and(|value| constraint.direction.passes(value, limit))));
            points.push(FrontierPoint {
                limit, at_declared_limit: limit == constraint.limit, feasible_count, winners, objective_mean,
                opportunity_cost: sacrifice(baseline_objective, objective_mean)?,
            });
        }
        frontiers.push(Frontier { constraint_id: constraint.id.clone(), points });
    }
    let summary = Summary {
        model_key: design.value.model_key.clone(),
        objective_unit: if program.inputs().equity.is_some() { "USD/share" } else { "USD" }.into(),
        baseline_winners, baseline_objective, constrained_winners, constrained_objective,
        opportunity_cost: sacrifice(baseline_objective, constrained_objective)?, choices, frontiers,
    };
    let design_json = crate::render::claim_json(&design, &render::design_json(&design.value));
    let summary_json = render::summary_json(&summary);
    let tag = crate::render::value_tag("capital-kpi-result/v1", &format!("{}|{design_json}|{summary_json}|{NO_CLAIM}", program.digest()));
    let mut metadata = Metadata::new();
    metadata.include(program.claim())?;
    metadata.include(&design)?;
    let mut claim = metadata.claim("capital.kpi-comparison/1", summary, &tag, AdditivityClass::NonAdditive, false);
    let inputs = program.inputs();
    claim.calibration_failed = design.calibration_failed
        || program.claim().calibration_failed
        || inputs.policy.calibration_failed
        || inputs.weights.calibration_failed
        || inputs.candidates.iter().any(|c| c.calibration_failed)
        || inputs.funding.as_ref().is_some_and(|c| c.calibration_failed)
        || inputs.funding_schedule.as_ref().is_some_and(|c| c.calibration_failed)
        || inputs.entity_funding.as_ref().is_some_and(|c| c.calibration_failed)
        || inputs.equity.as_ref().is_some_and(|c| c.calibration_failed)
        || inputs.entity_valuations.iter().any(|c| c.calibration_failed);
    let digest = sha256(crate::render::claim_json(&claim, &summary_json).as_bytes()).to_string();
    Ok(EvaluatedKpiProgram { program: program.clone(), design, claim, digest })
}
