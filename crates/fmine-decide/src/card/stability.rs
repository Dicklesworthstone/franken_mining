//! DESIGN (BD-229, §22.5): conditional selection stability of complete source-bound alternatives.
//! Determinism: Exact (seeded integer sampling and checked rational objective/risk comparisons).
//!
//! Structural feasibility is fixed over the original support. Resampling cannot waive an obligation
//! in an omitted state; it changes only weights, expected values and complete-distribution tail tests.

use super::*;
use fmine_numerics::risk::bootstrap::{BootstrapPlan, PreparedPayoffs, sample_counts};
use fmine_numerics::risk::exact::{ExactDistribution, compare};

mod render;
#[cfg(test)]
mod tests;

pub const METHOD: &str = "weighted_multinomial";
pub const MAX_SELECTION_WORK: usize = 32_000_000;
pub const MAX_RETAINED_WINNER_ID_BYTES: usize = 8_000_000;
pub const NO_CLAIM: &str = "Conditional weighted-bootstrap selection stability over the explicitly declared finite scenario law, seed and sample size. Counts reweight complete joint states; they are not empirical observations, an estimated effective sample size, calibrated probabilities of optimality, or uncertainty coverage for omitted regimes. Every original structural, cash, share-count and contractual constraint remains binding even when a state receives zero draws. Each replicate recomputes the primary expected objective and all declared tail-loss limits, preserving every exact co-winner. Static subsets and declared two-stage policies use the same paired resamples; policies retain their original information restrictions and initial obligations. Optional entity cash, debt, capital-call and valuation calculations retain their declared bases and are not repeated or inferred from resampling. Candidate inclusion and ranking-change bounds retain tie ambiguity; no-feasible-choice replicates remain in the denominator. All values remain conditional on source assumptions. No actual agreement terms, undeclared project interaction, additional decision stage, intra-period liquidity guarantee, historical validation, regulatory claim or execution authorization is supplied.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStabilityDesign {
    pub model_key: String,
    pub method: String,
    pub seed: u64,
    pub replicates: usize,
    pub draws_per_replicate: usize,
    pub sampling_basis: String,
}

impl SelectionStabilityDesign {
    #[must_use]
    pub fn plan(&self) -> BootstrapPlan {
        BootstrapPlan {
            seed: self.seed,
            replicates: self.replicates,
            draws_per_replicate: self.draws_per_replicate,
        }
    }

    #[must_use]
    pub fn canonical_tag(&self) -> String {
        format!(
            "selection-stability-design/v1:{}",
            sha256(self.to_json().as_bytes())
        )
    }

    /// Includes rejected alternatives in the bound before source serialization or enumeration.
    pub fn check_work(&self, states: usize, alternatives: usize, laws: usize) -> Result<()> {
        self.plan().validate(states)?;
        let work = self
            .replicates
            .checked_mul(states)
            .and_then(|n| n.checked_mul(alternatives))
            .and_then(|n| n.checked_mul(laws));
        if alternatives == 0
            || alternatives > 512
            || laws == 0
            || laws > 4
            || work.is_none_or(|n| n > MAX_SELECTION_WORK)
        {
            return Err(bad(
                "selection stability exceeds 32000000 shared-state law evaluations",
            ));
        }
        if self.method != METHOD || !text(&self.model_key) || !text(&self.sampling_basis) {
            return Err(bad(
                "selection stability needs weighted_multinomial, a model key and an explicit sampling basis",
            ));
        }
        Ok(())
    }
}

/// All laws use the enclosing problem's explicit USD or USD/share unit. Named secondary limits
/// remain separate constraints, never summed into the primary objective or into one another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilityTailLimit {
    pub name: String,
    pub values: Vec<Rat>,
    pub limit: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilityAlternative {
    pub alternative_id: String,
    pub structurally_feasible: bool,
    /// Empty for a structurally rejected alternative; its original source remains a Claim parent.
    pub values: Vec<Rat>,
    pub tail_limits: Vec<StabilityTailLimit>,
    /// Candidate bitsets, in the declared scope order. These are complete policies, not branch choices
    /// selected after seeing a replicate; zero-count states retain their original continuation.
    pub selections: Vec<u16>,
}

/// Aligns a named assessed law to the canonical comparison support without silently dropping rows.
/// Quantity units are retained by the enclosing source-bound stability problem.
pub fn ordered_values<'a>(
    states: &[(String, Rat)],
    values: impl Iterator<Item = (&'a str, Rat)>,
) -> Result<Vec<Rat>> {
    if !(1..=256).contains(&states.len()) {
        return Err(bad(
            "assessed stability laws require 1..=256 original states",
        ));
    }
    if states.iter().any(|(id, _)| !identifier(id))
        || states.windows(2).any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(bad(
            "assessed stability support needs canonical distinct state identifiers",
        ));
    }
    let mut by_state = BTreeMap::new();
    for (index, (id, value)) in values.enumerate() {
        if index >= states.len() {
            return Err(bad(
                "assessed stability law contains extra original-state rows",
            ));
        }
        if by_state.insert(id, value).is_some() {
            return Err(bad("assessed stability law repeats an original state"));
        }
    }
    if by_state.len() != states.len() {
        return Err(bad(
            "assessed stability law must retain the complete original support",
        ));
    }
    states
        .iter()
        .map(|(state, _)| {
            by_state
                .get(state.as_str())
                .copied()
                .ok_or_else(|| bad("assessed stability law lost an original state"))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionScope {
    pub scope: String,
    pub state_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStabilityProblem {
    pub design: SelectionStabilityDesign,
    pub states: Vec<(String, Rat)>,
    pub objective_unit: String,
    pub tail_mass: Rat,
    pub primary_tail_limit: Option<Rat>,
    pub candidate_ids: Vec<String>,
    pub scopes: Vec<SelectionScope>,
    pub alternatives: Vec<StabilityAlternative>,
}

impl SelectionStabilityProblem {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"STAB", 1);
        enc.put_str(&self.design.to_json());
        enc.put_str(&self.objective_unit);
        put_rat(&mut enc, self.tail_mass);
        enc.put_u8(u8::from(self.primary_tail_limit.is_some()));
        if let Some(limit) = self.primary_tail_limit {
            put_rat(&mut enc, limit);
        }
        enc.put_u64(self.states.len() as u64);
        for (id, weight) in &self.states {
            enc.put_str(id);
            put_rat(&mut enc, *weight);
        }
        enc.put_u64(self.candidate_ids.len() as u64);
        for id in &self.candidate_ids {
            enc.put_str(id);
        }
        enc.put_u64(self.scopes.len() as u64);
        for scope in &self.scopes {
            enc.put_str(&scope.scope);
            enc.put_u8(u8::from(scope.state_id.is_some()));
            if let Some(state) = &scope.state_id {
                enc.put_str(state);
            }
        }
        enc.put_u64(self.alternatives.len() as u64);
        for row in &self.alternatives {
            enc.put_str(&row.alternative_id);
            enc.put_u8(u8::from(row.structurally_feasible));
            enc.put_u64(row.values.len() as u64);
            for value in &row.values {
                put_rat(&mut enc, *value);
            }
            enc.put_u64(row.tail_limits.len() as u64);
            for limit in &row.tail_limits {
                enc.put_str(&limit.name);
                put_rat(&mut enc, limit.limit);
                enc.put_u64(limit.values.len() as u64);
                for value in &limit.values {
                    put_rat(&mut enc, *value);
                }
            }
            enc.put_u64(row.selections.len() as u64);
            for mask in &row.selections {
                enc.put_u64(u64::from(*mask));
            }
        }
        format!("selection-stability-problem/v1:{}", sha256(&enc.finish()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeFrequency {
    pub alternative_id: String,
    pub structurally_feasible: bool,
    pub nominal_winner: bool,
    pub risk_feasible_replicates: u32,
    pub co_winner_replicates: u32,
    pub selection_frequency: Rat,
    /// Only a nominal co-winner is flagged; losing alternatives are still reported with their frequency.
    pub fragile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFrequency {
    pub candidate_id: String,
    pub scope: String,
    pub state_id: Option<String>,
    pub certain_replicates: u32,
    pub possible_replicates: u32,
    pub frequency_lower: Rat,
    pub frequency_upper: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionReplicate {
    pub index: usize,
    pub state_counts: Vec<u32>,
    pub feasible_winners: Vec<String>,
    pub risk_feasible_alternatives: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionStabilitySummary {
    pub design: SelectionStabilityDesign,
    pub state_ids: Vec<String>,
    pub objective_unit: String,
    pub nominal_winners: Vec<String>,
    pub nominal_runner_up: Vec<String>,
    pub no_feasible_replicates: u32,
    pub definite_flip_frequency: Option<Rat>,
    pub possible_flip_frequency: Option<Rat>,
    pub alternatives: Vec<AlternativeFrequency>,
    pub candidate_frequencies: Vec<CandidateFrequency>,
    pub replicates: Vec<SelectionReplicate>,
}

fn bad(why: &str) -> Refusal {
    Refusal::new(RefusalCode::NoClaimInDomain201, why)
}

fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn exact<T>(value: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    value.map_err(|error| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            error.to_string(),
        )
    })
}

fn validate(problem: &SelectionStabilityProblem) -> Result<()> {
    let states = problem.states.len();
    let laws = problem
        .alternatives
        .iter()
        .map(|r| r.tail_limits.len() + 1)
        .max()
        .unwrap_or(1);
    problem
        .design
        .check_work(states, problem.alternatives.len(), laws)?;
    let retained_ids = problem
        .alternatives
        .iter()
        .try_fold(0_usize, |sum, row| {
            sum.checked_add(row.alternative_id.len())
        })
        .and_then(|bytes| bytes.checked_mul(problem.design.replicates));
    if retained_ids.is_none_or(|bytes| bytes > MAX_RETAINED_WINNER_ID_BYTES) {
        return Err(bad(
            "selection stability exceeds 8000000 retained winner-identifier bytes",
        ));
    }
    if !matches!(problem.objective_unit.as_str(), "USD" | "USD/share")
        || !(1..=8).contains(&problem.candidate_ids.len())
        || problem.tail_mass.signum() <= 0
        || compare(problem.tail_mass, ONE).is_gt()
        || problem.scopes.is_empty()
        || problem.scopes.len() > states + 1
    {
        return Err(bad(
            "selection stability requires explicit objective units, candidates, tail mass and bounded selection scopes",
        ));
    }
    let ordered = |ids: Vec<&str>| {
        ids.iter().all(|id| identifier(id)) && ids.windows(2).all(|pair| pair[0] < pair[1])
    };
    if !ordered(problem.states.iter().map(|(id, _)| id.as_str()).collect())
        || !ordered(problem.candidate_ids.iter().map(String::as_str).collect())
        || !ordered(
            problem
                .alternatives
                .iter()
                .map(|r| r.alternative_id.as_str())
                .collect(),
        )
    {
        return Err(bad(
            "selection stability requires canonical distinct state, candidate and alternative ids",
        ));
    }
    let mut total = ZERO;
    for (_, weight) in &problem.states {
        if weight.signum() <= 0 {
            return Err(bad(
                "selection stability needs positive original state weights",
            ));
        }
        total = exact(total.checked_add(*weight))?;
    }
    if total != ONE {
        return Err(bad("selection stability weights must sum exactly to one"));
    }
    let mut scopes = BTreeSet::new();
    for scope in &problem.scopes {
        let valid = match (scope.scope.as_str(), scope.state_id.as_deref()) {
            ("static" | "initial", None) => true,
            ("realized_state", Some(id)) => problem.states.iter().any(|(state, _)| state == id),
            _ => false,
        };
        if !valid || !scopes.insert((&scope.scope, &scope.state_id)) {
            return Err(bad(
                "selection scope is duplicated or lacks a declared state",
            ));
        }
    }
    let mask_limit = 1_u16 << problem.candidate_ids.len();
    for row in &problem.alternatives {
        if row.selections.len() != problem.scopes.len()
            || row.selections.iter().any(|mask| *mask >= mask_limit)
            || (row.structurally_feasible && row.values.len() != states)
            || (!row.structurally_feasible
                && (!row.values.is_empty() || !row.tail_limits.is_empty()))
        {
            return Err(bad(
                "selection alternative needs every scope and a complete law exactly when structurally feasible",
            ));
        }
        let mut names = BTreeSet::new();
        for limit in &row.tail_limits {
            if !text(&limit.name) || !names.insert(&limit.name) || limit.values.len() != states {
                return Err(bad(
                    "every secondary tail constraint needs a distinct name and the complete original support",
                ));
            }
        }
    }
    Ok(())
}

struct PreparedAlternative {
    primary: PreparedPayoffs,
    secondary: Vec<(PreparedPayoffs, Rat)>,
}

fn nominal(problem: &SelectionStabilityProblem, row: &StabilityAlternative) -> Result<Option<Rat>> {
    if !row.structurally_feasible {
        return Ok(None);
    }
    let distribution = |values: &[Rat]| {
        ExactDistribution::new(
            &values
                .iter()
                .zip(&problem.states)
                .map(|(value, (_, weight))| (*value, *weight))
                .collect::<Vec<_>>(),
        )
    };
    let primary = distribution(&row.values)?;
    // Evaluate fallible tails explicitly; a failed exact integral never becomes an admitted choice.
    if let Some(cap) = problem.primary_tail_limit
        && compare(primary.tail_loss(problem.tail_mass)?, cap).is_gt()
    {
        return Ok(None);
    }
    for limit in &row.tail_limits {
        if compare(
            distribution(&limit.values)?.tail_loss(problem.tail_mass)?,
            limit.limit,
        )
        .is_gt()
        {
            return Ok(None);
        }
    }
    Ok(Some(primary.mean()))
}

fn winning_indices(values: &[Option<Rat>], excluded: &[usize]) -> Vec<usize> {
    let best = values
        .iter()
        .enumerate()
        .filter(|(i, _)| !excluded.contains(i))
        .filter_map(|(_, value)| *value)
        .max_by(|a, b| compare(*a, *b));
    values
        .iter()
        .enumerate()
        .filter_map(|(i, value)| {
            (best.is_some() && *value == best && !excluded.contains(&i)).then_some(i)
        })
        .collect()
}

/// Assesses a frozen complete comparison population; the Claim binds the supplied original eligibility
/// and realized outcomes. The result is conditional robustness, not new evidence or calibrated inference.
pub fn evaluate(
    inputs: &Claim<SelectionStabilityProblem>,
) -> Result<Claim<SelectionStabilitySummary>> {
    let problem = &inputs.value;
    validate(problem)?;
    super::admit(
        inputs,
        &problem.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    let counts = sample_counts(
        &problem
            .states
            .iter()
            .map(|(_, weight)| *weight)
            .collect::<Vec<_>>(),
        problem.design.plan(),
    )?;
    let nominal_values = problem
        .alternatives
        .iter()
        .map(|row| nominal(problem, row))
        .collect::<Result<Vec<_>>>()?;
    let nominal_indices = winning_indices(&nominal_values, &[]);
    let ids = |indices: &[usize]| {
        indices
            .iter()
            .map(|i| problem.alternatives[*i].alternative_id.clone())
            .collect::<Vec<_>>()
    };
    let nominal_winners = ids(&nominal_indices);
    let nominal_runner_up = ids(&winning_indices(&nominal_values, &nominal_indices));
    let prepared = problem
        .alternatives
        .iter()
        .map(|row| {
            if !row.structurally_feasible {
                return Ok(None);
            }
            Ok(Some(PreparedAlternative {
                primary: PreparedPayoffs::new(&row.values)?,
                secondary: row
                    .tail_limits
                    .iter()
                    .map(|limit| Ok((PreparedPayoffs::new(&limit.values)?, limit.limit)))
                    .collect::<Result<Vec<_>>>()?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut alternatives = problem
        .alternatives
        .iter()
        .enumerate()
        .map(|(i, row)| AlternativeFrequency {
            alternative_id: row.alternative_id.clone(),
            structurally_feasible: row.structurally_feasible,
            nominal_winner: nominal_indices.contains(&i),
            risk_feasible_replicates: 0,
            co_winner_replicates: 0,
            selection_frequency: ZERO,
            fragile: false,
        })
        .collect::<Vec<_>>();
    let mut candidate_frequencies = problem
        .scopes
        .iter()
        .flat_map(|scope| {
            problem.candidate_ids.iter().map(|id| CandidateFrequency {
                candidate_id: id.clone(),
                scope: scope.scope.clone(),
                state_id: scope.state_id.clone(),
                certain_replicates: 0,
                possible_replicates: 0,
                frequency_lower: ZERO,
                frequency_upper: ZERO,
            })
        })
        .collect::<Vec<_>>();
    let (mut no_feasible_replicates, mut definite, mut possible) = (0_u32, 0_u32, 0_u32);
    let mut replicates = Vec::with_capacity(counts.len());
    for (index, state_counts) in counts.into_iter().enumerate() {
        let mut values = vec![None; prepared.len()];
        for (i, row) in prepared.iter().enumerate() {
            let Some(row) = row else {
                continue;
            };
            let primary = row.primary.evaluate(
                &state_counts,
                problem.primary_tail_limit.map(|_| problem.tail_mass),
            )?;
            if primary
                .tail_loss
                .zip(problem.primary_tail_limit)
                .is_some_and(|(tail, cap)| compare(tail, cap).is_gt())
            {
                continue;
            }
            let mut feasible = true;
            for (law, cap) in &row.secondary {
                let tail = law
                    .evaluate(&state_counts, Some(problem.tail_mass))?
                    .tail_loss
                    .ok_or_else(|| bad("secondary tail result is absent"))?;
                if compare(tail, *cap).is_gt() {
                    feasible = false;
                    break;
                }
            }
            if feasible {
                values[i] = Some(primary.mean);
                alternatives[i].risk_feasible_replicates += 1;
            }
        }
        let winners = winning_indices(&values, &[]);
        if winners.is_empty() {
            no_feasible_replicates += 1;
        } else {
            if !winners.iter().any(|i| nominal_indices.contains(i)) {
                definite += 1;
            }
            if winners.iter().any(|i| !nominal_indices.contains(i)) {
                possible += 1;
            }
            let (mut certain, mut possible_masks) = (
                vec![u16::MAX; problem.scopes.len()],
                vec![0_u16; problem.scopes.len()],
            );
            for winner in &winners {
                alternatives[*winner].co_winner_replicates += 1;
                for (scope, mask) in problem.alternatives[*winner].selections.iter().enumerate() {
                    certain[scope] &= mask;
                    possible_masks[scope] |= mask;
                }
            }
            for (scope, all) in certain.iter().enumerate() {
                for candidate in 0..problem.candidate_ids.len() {
                    let row =
                        &mut candidate_frequencies[scope * problem.candidate_ids.len() + candidate];
                    row.certain_replicates += u32::from(all & (1 << candidate) != 0);
                    row.possible_replicates +=
                        u32::from(possible_masks[scope] & (1 << candidate) != 0);
                }
            }
        }
        replicates.push(SelectionReplicate {
            index,
            state_counts,
            feasible_winners: ids(&winners),
            risk_feasible_alternatives: values.iter().filter(|v| v.is_some()).count(),
        });
    }
    let frequency = |count: u32| {
        exact(Rat::new(
            i128::from(count),
            problem.design.replicates as i128,
        ))
    };
    for row in &mut alternatives {
        row.selection_frequency = frequency(row.co_winner_replicates)?;
        row.fragile =
            row.nominal_winner && compare(row.selection_frequency, exact(Rat::new(4, 5))?).is_lt();
    }
    for row in &mut candidate_frequencies {
        row.frequency_lower = frequency(row.certain_replicates)?;
        row.frequency_upper = frequency(row.possible_replicates)?;
    }
    let summary = SelectionStabilitySummary {
        design: problem.design.clone(),
        state_ids: problem.states.iter().map(|(id, _)| id.clone()).collect(),
        objective_unit: problem.objective_unit.clone(),
        nominal_winners,
        nominal_runner_up,
        no_feasible_replicates,
        definite_flip_frequency: (!nominal_indices.is_empty())
            .then(|| frequency(definite))
            .transpose()?,
        possible_flip_frequency: (!nominal_indices.is_empty())
            .then(|| frequency(possible))
            .transpose()?,
        alternatives,
        candidate_frequencies,
        replicates,
    };
    let mut binding = Encoder::new_frame(*b"STBR", 1);
    binding.put_str(&super::render::claim_json(
        inputs,
        &super::render::js(&problem.canonical_tag()),
    ));
    binding.put_str(&summary.to_json());
    binding.put_str(NO_CLAIM);
    let tag = format!("selection-stability/v1:{}", sha256(&binding.finish()));
    Ok(Claim::new(
        "decision.selection-stability/1",
        summary,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        inputs.evidence.meet(EvidenceClass::Derived),
        inputs.epistemic,
        inputs.state,
        inputs.confidentiality.clone(),
        inputs.uu_terms.clone(),
        vec![inputs.claim_id],
        inputs.anchors.clone(),
    ))
}
