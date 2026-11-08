//! DESIGN (BD-229, §23): bounded exact static capital selection on a declared shared scenario index.
//! Determinism: Exact (inherited floating evidence metadata is bound by its exact bits).
//!
//! Spending constrains the budget; the supplied additive delta-NAV payoffs already include capital costs.
//! Neither marginal CVaRs nor individually optimal projects are combined into a portfolio risk estimate.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{
    AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport, UuTerm,
};
use fmine_core::ObjectId;
use fmine_core::anchor::SourceAnchor;
use fmine_core::codec::Encoder;
use fmine_core::hash::sha256;
use fmine_decide::card::{
    self, ActionPayoffs, ActionRole, AlternativeStats, DecisionInputs, ExactMoney, WeightSet,
};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::Rat;
use fmine_numerics::risk::exact::{ExactDistribution, compare as exact_compare};
use fmine_units::{Currency, Money, UnitsError};

pub mod entity;
pub mod model;
pub mod recourse;
mod render;
pub mod stability;

use entity::valuation::{EntityValuation, EntityValuationAssessment};
use entity::{EntityCashPhase, EntityFunding, EntityFundingAssessment};

pub const MAX_CANDIDATES: usize = 8;
pub const MAX_STATES: usize = 256;
pub const MAX_CASH_PERIODS: usize = 16;
/// All opening and period-end rows across every declared subset, including rejected subsets.
pub const MAX_CASH_CHECKPOINTS: usize = 65_536;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_IDENTIFIER_BYTES: usize = 128;
pub const PAYOFF_CONVENTION: &str = "additive_net_delta_nav";
pub const EQUITY_PAYOFF_CONVENTION: &str = "additive_equity_nav_with_share_changes";
pub const NO_CLAIM: &str = "Static comparison conditional on the declared complete shared-scenario model, additive net delta-NAV payoffs, spending constraints and reference-class adjustment bases. Optional funding checks cover only the explicitly declared cash window and basis; they do not guarantee every intra-period balance or a feasible staged schedule. Input weights are uncalibrated assumptions. The selected set is optimal only among the enumerated declared subsets. No project interactions, staged recourse, entity waterfalls, variable share counts, physical feasibility, retrospective evidence, selection-frequency estimate, regulatory claim or execution authorization is supplied. Planned costs and scenario cash requirements constrain funding and are not deducted again from net delta-NAV.";
pub const CASH_SCHEDULE_NO_CLAIM: &str = "Static comparison conditional on the declared complete shared-scenario model, additive net delta-NAV payoffs, spending constraints and reference-class adjustment bases. Cash checks cover the opening and period-end checkpoints of the declared cash schedule and basis; they do not guarantee intra-period balances or adaptive staged decisions. Input weights are uncalibrated assumptions. The selected set is optimal only among the enumerated declared subsets. No project interactions, staged recourse, entity waterfalls, variable share counts, physical feasibility, retrospective evidence, selection-frequency estimate, regulatory claim or execution authorization is supplied. Planned costs and declared cash flows constrain funding and are not charged to net delta-NAV again.";
pub const EQUITY_NO_CLAIM: &str = "Static comparison conditional on the declared complete shared-scenario model, true additive equity-NAV changes, signed share changes, spending constraints and reference-class adjustment bases. Selection uses the change in NAV per actual share for the named entity and share class; total-USD NAV statistics remain separate reporting columns. Optional funding checks cover only the explicitly declared cash window and basis, without guaranteeing intra-period balances. Share units are normalized exactly; no repurchase price, issuance proceeds or future share changes are inferred. Input weights are uncalibrated assumptions. The selected set is optimal only among the enumerated declared subsets. No staged recourse, entity waterfalls, calibration, selection-frequency estimate, regulatory claim or execution authorization is supplied. Planned costs and declared cash requirements constrain funding and are not charged to equity NAV again.";
pub const EQUITY_CASH_SCHEDULE_NO_CLAIM: &str = "Static comparison conditional on the declared complete shared-scenario model, true additive equity-NAV changes, signed share changes, spending constraints and reference-class adjustment bases. Selection uses the change in NAV per actual share for the named entity and share class; total-USD NAV statistics remain separate reporting columns. Cash checks cover opening and period-end checkpoints of the declared schedule and basis; they do not guarantee intra-period balances or adaptive staged decisions. Share units are normalized exactly; no repurchase price, issuance proceeds or future share changes are inferred. Input weights are uncalibrated assumptions. The selected set is optimal only among the enumerated declared subsets. No staged recourse, entity waterfalls, calibration, selection-frequency estimate, regulatory claim or execution authorization is supplied. Planned costs and declared cash flows constrain funding and are not charged to equity NAV again.";
pub const ENTITY_CASH_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios and net NAV attributable to the named objective entity. Entity cash stays in its own ledger except for explicitly declared fixed transfers and gated ownership distributions. Cash floors apply at initial opening, after native flows and fixed transfers, and after child-first distributions; earlier failures remain even after later receipts. The ownership remainder leaves the modeled graph as an explicit external distribution. Native flows include all declared outside partner contributions; no contribution, top-up, dilution, debt-service covenant, tax rule or distribution permission is inferred. Transfers and distributions constrain cash feasibility and are not added to or deducted from already-net NAV. Input probabilities, valuations and gates are uncalibrated assumptions. Selection is conditional on the complete bounded subset population; no intra-phase payment ordering, further valuation entity, historical validation, selection-frequency estimate, regulatory claim or execution authorization is supplied.";
pub const EQUITY_ENTITY_CASH_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios, additive equity-NAV changes and signed share changes for the named objective entity and share class. Selection uses exact change in NAV per actual share; total-USD NAV remains a separate reporting column. Entity cash stays in its own ledger except for declared fixed transfers and gated ownership distributions. Floors apply at initial opening, after native flows and transfers, and after child-first distributions; later receipts do not erase earlier failures. The ownership remainder leaves the graph as explicit external cash. No partner contribution, top-up, dilution, debt-service covenant, fiscal rule, distribution permission or second entity's NAV is inferred. Cash movements are not charged to already-net NAV again. Inputs remain uncalibrated assumptions. No intra-phase payment ordering, historical validation, selection-frequency estimate, regulatory claim or execution authorization is supplied.";
pub const ENTITY_DEBT_CASH_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios, net NAV for the objective entity, separate entity cash and explicit external senior debt terms. Native cash excludes modeled loan draws, interest, principal service and sweeps. Each period's declared rate applies to opening principal plus draws; full scheduled service is charged before cash-floor checks. Coverage uses separately declared CFADS divided by mandatory interest and principal, excluding sweeps; CFADS is not additional cash. Zero-service distribution permission is explicit. Child-first entities sweep available surplus before their own ownership distributions, subject to remaining principal. Dividends require both the declared gate and calculated coverage permission. Opening, pre-distribution and closing floors remain binding; later receipts do not erase earlier failures. Cash and principal conservation are exact. Debt cash does not change already-net NAV again. Inputs remain uncalibrated assumptions; no actual lending agreement, annualization, default remedy, arrears financing, covenant waiver, partner top-up, ownership dilution, second valuation denominator, historical validation, selection frequency, regulatory claim or execution authorization is inferred.";
pub const EQUITY_ENTITY_DEBT_CASH_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios, additive equity-NAV changes and signed share changes for the objective entity. Selection uses exact change in NAV per actual share, with separate total-USD statistics. Entity funding includes explicitly declared external senior debt: period rates on opening principal plus draws, full mandatory interest and principal service, separately declared CFADS coverage and floor-safe principal sweeps before each entity's child-first ownership distribution. Native cash excludes modeled debt cash, and CFADS is not another receipt. Zero-service distribution permission is explicit. All opening, pre-distribution and closing cash failures remain binding; cash and principal are exactly conserved. Debt cash movements are not applied to already-net equity NAV again. No actual loan terms, day-count rule, arrears, default remedy, covenant waiver, automatic ownership dilution, partner top-up or second entity valuation is inferred. Inputs remain uncalibrated assumptions; no historical validation, selection frequency, regulatory claim or execution authorization is supplied.";
pub const ENTITY_VALUATIONS_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios and separate complete equity-NAV and share-change matrices for the primary and secondary entities. Only the named primary entity's expected change in NAV per actual share determines selection. Every declared entity must retain positive ending shares; all statistics and hindsight comparisons use this jointly structurally feasible population. Optional entity-specific tail-loss limits apply to each complete unconditional shared-state distribution, and all limits bind jointly. Secondary values are separate reporting and constraint quantities, never weighted into the objective or added across overlapping valuation perimeters. Share units are normalized exactly. Optional aggregate or dated cash inputs constrain only their declared checkpoints and are not charged to already-net NAV again. No ownership-based NAV derivation, capital-call dilution, transaction reconciliation, undeclared project interaction, calibration, historical validation, selection frequency, regulatory claim or execution authorization is supplied. All probabilities and valuations remain uncalibrated source assumptions.";
pub const ENTITY_VALUATIONS_CASH_NO_CLAIM: &str = "Static comparison conditional on separate source-declared primary and secondary equity-NAV/share matrices over the same shared scenarios. The primary entity's expected per-share change is the sole objective. Every entity's ending shares must remain positive, and every declared per-share tail-loss limit binds the complete unconditional distribution; hindsight statistics use the jointly structurally feasible population before risk. Secondary valuation perimeters need not be cash entities, and their values are never inferred from ownership or summed into the primary objective. Entity cash follows explicit native flows, fixed transfers and gated child-first distributions. Opening, pre-distribution and closing floors remain binding even after later receipts; outside ownership payouts leave the graph. Cash movements are not charged to any already-net NAV again. No automatic dilution, partner contribution, fiscal rule, debt covenant, valuation consistency reconciliation, calibration, historical validation, selection frequency, regulatory claim or execution authorization is inferred. Inputs remain uncalibrated assumptions.";
pub const ENTITY_VALUATIONS_DEBT_NO_CLAIM: &str = "Static comparison conditional on separate source-declared primary and secondary equity-NAV/share matrices, shared scenarios, entity cash and explicit external senior debt terms. Only primary expected NAV/share change determines selection; all entities require positive ending shares and all declared per-share tail-loss limits bind jointly on complete unconditional outcomes. Secondary values are never inferred from cash or ownership, combined across overlapping perimeters, or treated as another objective. Native cash excludes modeled debt cash. Full-period interest on opening principal plus draws and mandatory principal service precede pre-distribution floor checks. Separately declared CFADS is not another receipt; coverage excludes sweeps and zero-service permission is explicit. Child-first sweeps include child receipts and apply to roots before ownership distributions. Earlier funding failures remain binding and cash/principal conservation is exact. Debt and other cash movements do not adjust already-net valuations again. No actual loan terms, annualization, default remedy, waiver, capital-call dilution, partner top-up, transaction reconciliation, calibration, historical validation, selection frequency, regulatory claim or execution authorization is inferred.";
pub const ENTITY_CAPITAL_CALL_NO_CLAIM: &str = "Static comparison conditional on declared shared scenarios, separate entity cash and explicit pre-money subscription terms. Native cash excludes modeled capital-call contributions. Each active call debits the full declared parent contribution and credits the JV with parent plus outside funding; ownership becomes (prior parent share times declared pre-money value plus parent contribution) divided by pre-money value plus all contributions. Explicit sequences govern multiple calls within an entity-period. Updated ownership applies to that period's and later child-first distributions. Calls settle before any modeled debt service and pre-distribution cash-floor checks. A funding shortfall remains a structural rejection even when existing JV cash covers expenditures; unaffordable parent contributions are never clipped. Inactive and rejected calls remain visible. Optional debt retains its declared coverage and sweep rules. Selection uses the primary already-net NAV objective, or its per-share change when equity is declared; every declared secondary share-count and tail-loss constraint remains binding. Cash and ownership calculations never infer or adjust those independent valuations or share counts. Inputs remain uncalibrated assumptions; no actual JV agreement, penalty dilution, automatic top-up, subscription rollback, intra-phase liquidity guarantee, transaction reconciliation, historical validation, regulatory claim or execution authorization is supplied.";
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPolicy {
    pub model_key: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub budget: Money,
    /// Upper tail mass of signed loss = -delta NAV; 1/5 means worst 20%.
    pub tail_mass: Rat,
    pub max_tail_loss: Option<Money>,
    pub payoff_convention: String,
    pub reference_class_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalCandidate {
    pub model_key: String,
    pub candidate_id: String,
    pub label: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub cost: Money,
    pub payoffs: Vec<(String, Money)>,
    pub requires: Vec<String>,
    pub exclusion_group: Option<String>,
    pub reference_class_basis: String,
}

/// Cash required in one declared window, independently of the candidate's net-NAV valuation basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashRequirement {
    pub candidate_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramFunding {
    pub model_key: String,
    pub currency: Currency,
    /// Inclusive canonical date range YYYY-MM-DD/YYYY-MM-DD; no calendar is inferred.
    pub cash_period: String,
    pub cash_basis: String,
    pub minimum_cash: Money,
    /// Cash available before this program's declared requirements, one explicit amount per shared state.
    pub available_cash: Vec<(String, Money)>,
    /// Complete candidate-by-state cash requirements, including explicit zeroes.
    pub requirements: Vec<CashRequirement>,
}

impl ProgramFunding {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-funding/v1", &render::funding_json(self))
    }
}

/// A declared inclusive cash period; floors are checked at its closing date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashPeriod {
    pub id: String,
    pub cash_period: String,
    pub minimum_cash: Money,
}

/// Signed cash movement outside the selected program: positive receipts, negative expenditures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCashFlow {
    pub period_id: String,
    pub state_id: String,
    pub value: Money,
}

/// Signed incremental cash movement of a candidate, independently of its already-net NAV payoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCashFlow {
    pub period_id: String,
    pub candidate_id: String,
    pub state_id: String,
    pub value: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingSchedule {
    pub model_key: String,
    pub currency: Currency,
    pub cash_basis: String,
    pub opening_date: String,
    pub opening_cash: Vec<(String, Money)>,
    pub periods: Vec<CashPeriod>,
    pub baseline_flows: Vec<StateCashFlow>,
    pub candidate_flows: Vec<CandidateCashFlow>,
}

impl FundingSchedule {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag(
            "capital-funding-schedule/v1",
            &render::funding_schedule_json(self),
        )
    }
}

/// Signed change in declared share units: positive issuance, negative retirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateShareChange {
    pub candidate_id: String,
    pub state_id: String,
    pub units: Rat,
}

/// Equity NAV and share counts refer to one explicit entity, share class and valuation basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquityBasis {
    pub model_key: String,
    pub currency: Currency,
    pub valuation_basis: String,
    pub entity: String,
    pub share_class: String,
    pub shares_per_unit: Rat,
    pub initial_share_units: Rat,
    pub baseline_nav: Vec<(String, Money)>,
    pub share_changes: Vec<CandidateShareChange>,
    /// Signed upper-tail loss in major currency per actual share, independently of total-USD limits.
    pub max_tail_loss_per_share: Option<Rat>,
}

impl EquityBasis {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-equity/v1", &render::equity_json(self))
    }
}

/// Exact major currency per actual share; this quantity is distinct from total money.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactPerShare {
    pub currency: Currency,
    pub major: Rat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerShareStateResult {
    pub state_id: String,
    pub weight: Rat,
    pub payoff: ExactPerShare,
    pub best_in_hindsight: ExactPerShare,
    pub regret: ExactPerShare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerShareStats {
    pub action_id: String,
    pub label: String,
    pub role: ActionRole,
    pub mean: ExactPerShare,
    pub p10_p50_p90: [ExactPerShare; 3],
    pub cvar95_loss: ExactPerShare,
    pub mean_regret: ExactPerShare,
    pub max_regret: ExactPerShare,
    pub states: Vec<PerShareStateResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquityState {
    pub state_id: String,
    pub baseline_nav: ExactMoney,
    pub nav_change: ExactMoney,
    pub ending_nav: ExactMoney,
    pub baseline_shares: Rat,
    pub share_change: Rat,
    pub ending_shares: Rat,
    pub baseline_nav_per_share: ExactPerShare,
    pub ending_nav_per_share: Option<ExactPerShare>,
    pub delta_nav_per_share: Option<ExactPerShare>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquityAssessment {
    pub states: Vec<EquityState>,
}

impl ProgramPolicy {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-policy/v1", &render::policy_json(self))
    }
}
impl CapitalCandidate {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        render::value_tag("capital-candidate/v1", &render::candidate_json(self))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgramInputs {
    pub policy: Claim<ProgramPolicy>,
    pub weights: Claim<WeightSet>,
    pub candidates: Vec<Claim<CapitalCandidate>>,
    pub funding: Option<Claim<ProgramFunding>>,
    pub funding_schedule: Option<Claim<FundingSchedule>>,
    pub entity_funding: Option<Claim<EntityFunding>>,
    pub equity: Option<Claim<EquityBasis>>,
    pub entity_valuations: Vec<Claim<EntityValuation>>,
    pub selection_stability: Option<Claim<stability::SelectionStabilityDesign>>,
}

fn no_claim_for(inputs: &ProgramInputs) -> &'static str {
    if inputs.selection_stability.is_some() {
        return stability::NO_CLAIM;
    }
    if inputs
        .entity_funding
        .as_ref()
        .is_some_and(|claim| claim.value.capital_calls.is_some())
    {
        return ENTITY_CAPITAL_CALL_NO_CLAIM;
    }
    if !inputs.entity_valuations.is_empty() {
        return match inputs.entity_funding.as_ref() {
            Some(claim) if claim.value.debt.is_some() => ENTITY_VALUATIONS_DEBT_NO_CLAIM,
            Some(_) => ENTITY_VALUATIONS_CASH_NO_CLAIM,
            None => ENTITY_VALUATIONS_NO_CLAIM,
        };
    }
    if inputs
        .entity_funding
        .as_ref()
        .is_some_and(|claim| claim.value.debt.is_some())
    {
        return if inputs.equity.is_some() {
            EQUITY_ENTITY_DEBT_CASH_NO_CLAIM
        } else {
            ENTITY_DEBT_CASH_NO_CLAIM
        };
    }
    if inputs.entity_funding.is_some() {
        return if inputs.equity.is_some() {
            EQUITY_ENTITY_CASH_NO_CLAIM
        } else {
            ENTITY_CASH_NO_CLAIM
        };
    }
    match (inputs.equity.is_some(), inputs.funding_schedule.is_some()) {
        (false, false) => NO_CLAIM,
        (false, true) => CASH_SCHEDULE_NO_CLAIM,
        (true, false) => EQUITY_NO_CLAIM,
        (true, true) => EQUITY_CASH_SCHEDULE_NO_CLAIM,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralRejection {
    BudgetExceeded,
    MissingPrerequisite {
        candidate: String,
        prerequisite: String,
    },
    MutualExclusion {
        group: String,
        candidates: Vec<String>,
    },
    LiquidityShortfall {
        state_id: String,
        shortfall: ExactMoney,
    },
    CashScheduleShortfall {
        /// `None` identifies the initial opening checkpoint.
        period_id: Option<String>,
        checkpoint_date: String,
        state_id: String,
        shortfall: ExactMoney,
    },
    EntityCashShortfall {
        entity_id: String,
        state_id: String,
        period_id: Option<String>,
        checkpoint_date: String,
        phase: EntityCashPhase,
        shortfall: ExactMoney,
    },
    EntityCapitalCallShortfall {
        entity_id: String,
        call_id: String,
        state_id: String,
        period_id: String,
        checkpoint_date: String,
        shortfall: ExactMoney,
    },
    NonpositiveShareCount {
        state_id: String,
        shares: Rat,
    },
    EntityNonpositiveShareCount {
        entity: String,
        share_class: String,
        state_id: String,
        shares: Rat,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingState {
    pub state_id: String,
    pub available_cash: ExactMoney,
    pub cash_required: ExactMoney,
    pub remaining_cash: ExactMoney,
    pub minimum_cash: ExactMoney,
    pub slack: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingAssessment {
    pub states: Vec<FundingState>,
    pub minimum_slack: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashScheduleCheckpoint {
    /// `None` identifies the initial opening checkpoint, whose two flow values are zero.
    pub period_id: Option<String>,
    pub checkpoint_date: String,
    pub state_id: String,
    pub opening_cash: ExactMoney,
    pub baseline_cash_flow: ExactMoney,
    pub candidate_cash_flow: ExactMoney,
    pub closing_cash: ExactMoney,
    pub minimum_cash: ExactMoney,
    pub slack: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingScheduleAssessment {
    pub checkpoints: Vec<CashScheduleCheckpoint>,
    pub minimum_slack: ExactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramChoice {
    pub bundle_id: String,
    pub selected: Vec<String>,
    pub spend: ExactMoney,
    pub budget_slack: ExactMoney,
    /// Present for every subset when funding is declared, including structurally rejected subsets.
    pub funding: Option<FundingAssessment>,
    /// Complete checkpoint results, also retained for structurally rejected subsets.
    pub funding_schedule: Option<FundingScheduleAssessment>,
    pub entity_funding: Option<EntityFundingAssessment>,
    pub equity: Option<EquityAssessment>,
    pub entity_valuations: Vec<EntityValuationAssessment>,
    pub structural_rejections: Vec<StructuralRejection>,
    /// None for a structurally rejected subset; no rejected subset is silently removed.
    pub statistics: Option<AlternativeStats>,
    pub tail_loss: Option<ExactMoney>,
    pub per_share_statistics: Option<PerShareStats>,
    pub per_share_tail_loss: Option<ExactPerShare>,
    pub risk_rejected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSummary {
    pub model_key: String,
    pub question: String,
    pub policy: ProgramPolicy,
    pub funding: Option<ProgramFunding>,
    pub funding_schedule: Option<FundingSchedule>,
    pub entity_funding: Option<EntityFunding>,
    pub equity: Option<EquityBasis>,
    pub entity_valuations: Vec<EntityValuation>,
    pub choices: Vec<ProgramChoice>,
    /// Budget, prerequisites, exclusions and declared funding applied, before the optional tail-loss limit.
    pub unconstrained_winners: Vec<String>,
    pub feasible_winners: Vec<String>,
    /// Absent when no structurally/risk-feasible subset exists; negative values cannot occur.
    pub expected_value_cost_of_risk_limit: Option<ExactMoney>,
    pub expected_per_share_cost_of_risk_limit: Option<ExactPerShare>,
    pub selection_stability: Option<stability::SelectionStabilitySummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedProgram {
    inputs: ProgramInputs,
    claim: Claim<ProgramSummary>,
    digest: String,
}
impl EvaluatedProgram {
    #[must_use]
    pub fn inputs(&self) -> &ProgramInputs {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &ProgramSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<ProgramSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
    #[must_use]
    pub fn no_claim(&self) -> &'static str {
        no_claim_for(&self.inputs)
    }
}

fn bad(code: RefusalCode, why: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("capital program: {}", why.into()))
}
fn arithmetic<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|e| bad(RefusalCode::NumericalCertificateFailed207, e.to_string()))
}
fn named(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
fn key(value: &str) -> bool {
    named(value)
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
fn amount(value: Money) -> Result<Rat> {
    arithmetic(Rat::new(value.minor, Money::SCALE))
}
fn checked_minor_add(a: i128, b: i128) -> Result<i128> {
    a.checked_add(b).ok_or_else(|| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "monetary sum overflowed",
        )
    })
}

fn admit<T>(claim: &Claim<T>, tag: &str, additivity: AdditivityClass) -> Result<()> {
    claim.check_id()?;
    if claim.value_tag() != tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "typed input differs from its canonical value tag",
        ));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale input"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) {
        return Err(bad(
            RefusalCode::NotObservable202,
            "input state is unusable",
        ));
    }
    if claim.representation != Representation::Deterministic
        || claim.support != SpatialSupport::PeriodAggregate
        || claim.additivity != additivity
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "inputs require deterministic period aggregates with explicit monetary or non-additive support",
        ));
    }
    if !named(&claim.metric_key)
        || claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every input needs a named Claim and source digest/locator",
        ));
    }
    claim.verify_uu_slot_for_decision()?;
    if claim.uu_terms.is_empty()
        || claim.uu_terms.iter().any(|u| {
            !key(&u.scenario_id)
                || !named(&u.description)
                || !named(&u.trigger_indicator)
                || !u.downside_exposure_usd_m.is_finite()
                || u.downside_exposure_usd_m <= 0.0
        })
    {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "each input needs named UU scenarios with positive finite exposure and a trigger",
        ));
    }
    validate_color_payload(&claim.color()?).map_err(|e| {
        bad(
            RefusalCode::CalibrationRequired205,
            format!("invalid input colour: {e:?}"),
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
    parents: BTreeSet<ObjectId>,
    anchors: BTreeSet<SourceAnchor>,
}
impl Metadata {
    fn new() -> Self {
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
    fn include<T>(&mut self, claim: &Claim<T>) -> Result<()> {
        self.evidence = self.evidence.meet(claim.evidence);
        self.epistemic = self.epistemic.and(claim.epistemic);
        self.state = self.state.and(claim.state);
        self.confidentiality = self.confidentiality.join(&claim.confidentiality);
        self.parents.insert(claim.claim_id);
        self.anchors.extend(claim.anchors.iter().cloned());
        for u in &claim.uu_terms {
            if let Some(old) = self.uu.insert(u.scenario_id.clone(), u.clone())
                && old != *u
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    format!("UU {} has conflicting definitions", u.scenario_id),
                ));
            }
        }
        Ok(())
    }
    fn claim<T>(
        &self,
        metric: &str,
        value: T,
        tag: &str,
        additivity: AdditivityClass,
        compact: bool,
    ) -> Claim<T> {
        // Internal bundle calculations keep every input parent and one anchor per source artifact. The
        // public result below keeps every original locator and full input Claim; avoid copying thousands
        // of identical source locators into each of the 256 temporary bundle inputs.
        let anchors = if compact {
            let mut sources = BTreeMap::new();
            for anchor in &self.anchors {
                sources
                    .entry((&anchor.artifact_path, anchor.artifact_sha256))
                    .or_insert_with(|| anchor.clone());
            }
            sources.into_values().collect()
        } else {
            self.anchors.iter().cloned().collect()
        };
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
            anchors,
        )
    }
}

fn validate_funding(
    claim: &Claim<ProgramFunding>,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    let funding = &claim.value;
    if funding.available_cash.len() != states.len()
        || funding.requirements.len() != candidates.len() * states.len()
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "funding requires every shared state's available cash and every candidate/state cash requirement, including zeroes",
        ));
    }
    admit(
        claim,
        &funding.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if funding.model_key != policy.model_key
        || funding.currency != policy.currency
        || !named(&funding.cash_basis)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "funding must name the same model/currency and a separate explicit cash basis",
        ));
    }
    let period_ok = funding
        .cash_period
        .split_once('/')
        .is_some_and(|(start, end)| {
            fmine_registry::sources::is_date(start)
                && fmine_registry::sources::is_date(end)
                && start <= end
        });
    if !period_ok {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "cash_period must be an ordered inclusive YYYY-MM-DD/YYYY-MM-DD range of valid dates",
        ));
    }
    let valid_money = |value: Money| value.ccy == policy.currency && value.minor >= 0;
    if !valid_money(funding.minimum_cash) {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "minimum cash must be nonnegative USD",
        ));
    }
    let mut present = BTreeSet::new();
    for (state, cash) in &funding.available_cash {
        if !states.contains(state.as_str())
            || !present.insert(state.as_str())
            || !valid_money(*cash)
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "available cash needs each shared state once, with nonnegative USD amounts",
            ));
        }
    }
    let mut pairs = BTreeSet::new();
    for required in &funding.requirements {
        if !candidates.contains_key(required.candidate_id.as_str())
            || !states.contains(required.state_id.as_str())
            || !pairs.insert((required.candidate_id.as_str(), required.state_id.as_str()))
            || !valid_money(required.value)
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "cash requirements need each known candidate/shared-state pair once, with nonnegative USD amounts",
            ));
        }
    }
    Ok(())
}

fn period_dates(period: &str) -> Option<(&str, &str)> {
    if period.len() != 21 {
        return None;
    }
    let (start, end) = period.split_once('/')?;
    (fmine_registry::sources::is_date(start)
        && fmine_registry::sources::is_date(end)
        && start <= end)
        .then_some((start, end))
}

/// The next ISO calendar date, solely for checking contiguous declared periods.
fn next_date(date: &str) -> Option<String> {
    if !fmine_registry::sources::is_date(date) {
        return None;
    }
    let mut year: u16 = date.get(..4)?.parse().ok()?;
    let mut month: u8 = date.get(5..7)?.parse().ok()?;
    let mut day: u8 = date.get(8..)?.parse().ok()?;
    let last = match month {
        2 if year.is_multiple_of(400) || year.is_multiple_of(4) && !year.is_multiple_of(100) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day < last {
        day += 1;
    } else {
        day = 1;
        if month < 12 {
            month += 1;
        } else {
            month = 1;
            year = year.checked_add(1)?;
            if year > 9999 {
                return None;
            }
        }
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn validate_schedule_bounds(
    schedule: &FundingSchedule,
    state_count: usize,
    candidate_count: usize,
) -> Result<()> {
    // Bound typed matrices before canonical serialization or metadata hashing allocates any copies.
    if !(1..=MAX_CASH_PERIODS).contains(&schedule.periods.len()) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "cash schedules require 1..=16 declared periods",
        ));
    }
    let checkpoint_count = (1_usize << candidate_count)
        .checked_mul(state_count)
        .and_then(|n| n.checked_mul(schedule.periods.len() + 1));
    if checkpoint_count.is_none_or(|count| count > MAX_CASH_CHECKPOINTS) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "cash schedules may produce at most 65536 opening and period-end checkpoints across all declared subsets",
        ));
    }
    let state_periods = state_count * schedule.periods.len();
    if schedule.opening_cash.len() != state_count
        || schedule.baseline_flows.len() != state_periods
        || schedule.candidate_flows.len() != state_periods * candidate_count
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "cash schedules require every opening state, period/state baseline flow and period/candidate/state flow, including zeroes",
        ));
    }
    Ok(())
}

fn validate_schedule(
    claim: &Claim<FundingSchedule>,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    let schedule = &claim.value;
    if schedule.model_key != policy.model_key
        || schedule.currency != policy.currency
        || !named(&schedule.cash_basis)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "cash schedules require the policy model/currency and an explicit cash basis",
        ));
    }
    if !fmine_registry::sources::is_date(&schedule.opening_date) {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "cash schedule opening_date must be a valid YYYY-MM-DD date",
        ));
    }
    let valid_nonnegative = |value: Money| value.ccy == policy.currency && value.minor >= 0;
    let mut period_ids = BTreeSet::new();
    let mut ordered = Vec::with_capacity(schedule.periods.len());
    for period in &schedule.periods {
        if !key(&period.id)
            || !period_ids.insert(period.id.as_str())
            || !valid_nonnegative(period.minimum_cash)
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "cash periods require distinct bounded identifiers and nonnegative USD floors",
            ));
        }
        let (start, end) = period_dates(&period.cash_period).ok_or_else(|| {
            bad(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "cash periods require ordered inclusive YYYY-MM-DD/YYYY-MM-DD dates",
            )
        })?;
        ordered.push((start, end));
    }
    ordered.sort_unstable();
    let mut expected_start = schedule.opening_date.clone();
    for (index, (start, end)) in ordered.iter().enumerate() {
        if *start != expected_start {
            return Err(bad(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "cash periods must be contiguous from opening_date, without gaps or overlaps",
            ));
        }
        if index + 1 < ordered.len() {
            expected_start = next_date(end).ok_or_else(|| {
                bad(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    "cash period end has no representable next calendar date",
                )
            })?;
        }
    }
    let mut opening_states = BTreeSet::new();
    for (state, cash) in &schedule.opening_cash {
        if !states.contains(state.as_str())
            || !opening_states.insert(state.as_str())
            || !valid_nonnegative(*cash)
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "opening cash requires every shared state once with nonnegative USD amounts",
            ));
        }
    }
    let mut pairs = BTreeSet::new();
    for flow in &schedule.baseline_flows {
        if !period_ids.contains(flow.period_id.as_str())
            || !states.contains(flow.state_id.as_str())
            || !pairs.insert((flow.period_id.as_str(), flow.state_id.as_str()))
            || flow.value.ccy != policy.currency
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "baseline cash flows require each known period/state once in USD",
            ));
        }
    }
    let mut triples = BTreeSet::new();
    for flow in &schedule.candidate_flows {
        if !period_ids.contains(flow.period_id.as_str())
            || !candidates.contains_key(flow.candidate_id.as_str())
            || !states.contains(flow.state_id.as_str())
            || !triples.insert((
                flow.period_id.as_str(),
                flow.candidate_id.as_str(),
                flow.state_id.as_str(),
            ))
            || flow.value.ccy != policy.currency
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "candidate cash flows require each known period/candidate/state once in USD",
            ));
        }
    }
    admit(
        claim,
        &schedule.canonical_tag(),
        AdditivityClass::NonAdditive,
    )
}

fn validate_equity_bounds(
    equity: &EquityBasis,
    state_count: usize,
    candidate_count: usize,
) -> Result<()> {
    if equity.baseline_nav.len() != state_count
        || equity.share_changes.len() != state_count * candidate_count
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "equity requires every baseline-NAV state and every candidate/state share change, including zeroes",
        ));
    }
    Ok(())
}

fn validate_equity(
    claim: &Claim<EquityBasis>,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
) -> Result<()> {
    validate_equity_values(&claim.value, policy, states, candidates, true)?;
    admit(
        claim,
        &claim.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )
}

fn validate_equity_values(
    equity: &EquityBasis,
    policy: &ProgramPolicy,
    states: &BTreeSet<&str>,
    candidates: &BTreeMap<&str, usize>,
    primary: bool,
) -> Result<()> {
    if equity.model_key != policy.model_key
        || equity.currency != policy.currency
        || (primary && equity.valuation_basis != policy.valuation_basis)
        || !named(&equity.valuation_basis)
        || !named(&equity.entity)
        || !named(&equity.share_class)
        || policy.max_tail_loss.is_some()
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            if primary {
                "equity requires the policy model/currency/valuation basis, a named entity/share class, and no total-USD tail-loss cap"
            } else {
                "secondary equity requires the policy model/currency, its own named valuation basis/entity/share class, and no total-USD tail-loss cap"
            },
        ));
    }
    if equity.shares_per_unit.signum() <= 0 || equity.initial_share_units.signum() <= 0 {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "share-unit scale and initial share units must be strictly positive",
        ));
    }
    arithmetic(
        equity
            .initial_share_units
            .checked_mul(equity.shares_per_unit),
    )?;
    let mut present = BTreeSet::new();
    for (state, value) in &equity.baseline_nav {
        if !states.contains(state.as_str())
            || !present.insert(state.as_str())
            || value.ccy != policy.currency
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "baseline equity NAV requires each shared state once in the policy currency",
            ));
        }
    }
    let mut pairs = BTreeSet::new();
    for change in &equity.share_changes {
        if !candidates.contains_key(change.candidate_id.as_str())
            || !states.contains(change.state_id.as_str())
            || !pairs.insert((change.candidate_id.as_str(), change.state_id.as_str()))
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "share changes require each known candidate/shared-state pair once",
            ));
        }
    }
    Ok(())
}

fn validate(inputs: &ProgramInputs) -> Result<Metadata> {
    if !(1..=MAX_CANDIDATES).contains(&inputs.candidates.len())
        || !(1..=MAX_STATES).contains(&inputs.weights.value.states.len())
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "requires 1..=8 candidates and 1..=256 shared states",
        ));
    }
    entity::valuation::check_work_bound(inputs, 1_usize << inputs.candidates.len())?;
    stability::check_work(inputs, 1_usize << inputs.candidates.len())?;
    if !inputs.entity_valuations.is_empty() && inputs.equity.is_none() {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "secondary entity valuations require one explicit primary equity objective",
        ));
    }
    for claim in &inputs.entity_valuations {
        entity::valuation::validate_bounds(
            &claim.value,
            inputs.weights.value.states.len(),
            inputs.candidates.len(),
        )?;
    }
    if let Some(schedule) = &inputs.funding_schedule {
        validate_schedule_bounds(
            &schedule.value,
            inputs.weights.value.states.len(),
            inputs.candidates.len(),
        )?;
    }
    if let Some(funding) = &inputs.entity_funding {
        entity::validate_bounds(
            &funding.value,
            inputs.weights.value.states.len(),
            inputs.candidates.len(),
        )?;
    }
    if let Some(equity) = &inputs.equity {
        validate_equity_bounds(
            &equity.value,
            inputs.weights.value.states.len(),
            inputs.candidates.len(),
        )?;
    }
    let policy = &inputs.policy.value;
    admit(
        &inputs.policy,
        &policy.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    admit(
        &inputs.weights,
        &inputs.weights.value.canonical_tag(),
        AdditivityClass::NonAdditive,
    )?;
    if !key(&policy.model_key)
        || policy.model_key != inputs.weights.value.model_key
        || !named(&inputs.weights.value.question)
        || !named(&policy.valuation_basis)
        || !named(&policy.reference_class_basis)
        || policy.payoff_convention
            != if inputs.equity.is_some() {
                EQUITY_PAYOFF_CONVENTION
            } else {
                PAYOFF_CONVENTION
            }
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "a common named model, valuation basis, reference-class basis and the explicit payoff convention for the declared objective are required",
        ));
    }
    if policy.currency != Currency::USD
        || policy.budget.ccy != policy.currency
        || policy
            .max_tail_loss
            .is_some_and(|v| v.ccy != policy.currency)
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "this slice uses explicit USD major units throughout; no currency conversion",
        ));
    }
    if policy.budget.minor < 0
        || policy.tail_mass.signum() <= 0
        || arithmetic(policy.tail_mass.checked_cmp(ONE))?.is_gt()
    {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "budget must be nonnegative and tail mass must lie in (0,1]",
        ));
    }
    let mut states = BTreeSet::new();
    let mut weight_sum = ZERO;
    for (id, weight) in &inputs.weights.value.states {
        if !key(id) || !states.insert(id.as_str()) || weight.signum() <= 0 {
            return Err(bad(
                RefusalCode::OutsidePhysicalDomain109,
                "distinct named states need strictly positive exact weights",
            ));
        }
        weight_sum = arithmetic(weight_sum.checked_add(*weight))?;
    }
    if weight_sum != ONE {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "weights must sum exactly to one; no implicit normalization",
        ));
    }
    let mut metadata = Metadata::new();
    metadata.include(&inputs.policy)?;
    metadata.include(&inputs.weights)?;
    if let Some(claim) = &inputs.selection_stability {
        admit(
            claim,
            &claim.value.canonical_tag(),
            AdditivityClass::NonAdditive,
        )?;
        if claim.value.model_key != policy.model_key {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "selection stability must use the program's model key",
            ));
        }
        metadata.include(claim)?;
    }
    let mut candidates = BTreeMap::new();
    for (i, claim) in inputs.candidates.iter().enumerate() {
        let c = &claim.value;
        admit(
            claim,
            &c.canonical_tag(),
            AdditivityClass::ExtensiveAdditive,
        )?;
        metadata.include(claim)?;
        if !key(&c.candidate_id)
            || !named(&c.label)
            || !named(&c.reference_class_basis)
            || c.exclusion_group.as_deref().is_some_and(|g| !key(g))
            || candidates.insert(c.candidate_id.as_str(), i).is_some()
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "candidate ids must be unique; labels, adjustment bases and any exclusion-group names are required",
            ));
        }
        if c.model_key != policy.model_key
            || c.currency != policy.currency
            || c.valuation_basis != policy.valuation_basis
            || c.cost.ccy != policy.currency
            || c.cost.minor < 0
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "candidate model, currency, valuation basis and nonnegative spending must agree with policy",
            ));
        }
        let mut present = BTreeSet::new();
        for (id, payoff) in &c.payoffs {
            if !states.contains(id.as_str())
                || !present.insert(id.as_str())
                || payoff.ccy != policy.currency
            {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "duplicate/foreign payoff state or currency mismatch",
                ));
            }
        }
        if present != states {
            return Err(bad(
                RefusalCode::NoClaimInDomain201,
                format!("{} lacks a payoff for every declared state", c.candidate_id),
            ));
        }
    }
    let mut reach = vec![0_u16; inputs.candidates.len()];
    for (i, c) in inputs.candidates.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for required in &c.value.requires {
            let j = candidates.get(required.as_str()).ok_or_else(|| {
                bad(
                    RefusalCode::BasisMismatch105,
                    format!("unknown prerequisite {required}"),
                )
            })?;
            if !seen.insert(required) {
                return Err(bad(RefusalCode::BasisMismatch105, "duplicate prerequisite"));
            }
            reach[i] |= 1_u16 << j;
        }
    }
    for k in 0..reach.len() {
        for i in 0..reach.len() {
            if reach[i] & (1 << k) != 0 {
                let next = reach[k];
                reach[i] |= next;
            }
        }
    }
    if reach
        .iter()
        .enumerate()
        .any(|(i, mask)| mask & (1 << i) != 0)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "prerequisite graph contains a cycle",
        ));
    }
    if usize::from(inputs.funding.is_some())
        + usize::from(inputs.funding_schedule.is_some())
        + usize::from(inputs.entity_funding.is_some())
        > 1
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "declare only one of aggregate funding, a cash schedule or entity funding",
        ));
    }
    if let Some(funding) = &inputs.funding {
        validate_funding(funding, policy, &states, &candidates)?;
        metadata.include(funding)?;
    }
    if let Some(schedule) = &inputs.funding_schedule {
        validate_schedule(schedule, policy, &states, &candidates)?;
        metadata.include(schedule)?;
    }
    if let Some(funding) = &inputs.entity_funding {
        entity::validate(funding, policy, &states, &candidates)?;
        metadata.include(funding)?;
        if inputs
            .equity
            .as_ref()
            .is_some_and(|equity| equity.value.entity != funding.value.objective_entity)
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "equity valuation and entity funding must name the same objective entity",
            ));
        }
    }
    if let Some(equity) = &inputs.equity {
        validate_equity(equity, policy, &states, &candidates)?;
        metadata.include(equity)?;
        let mut valuation_entities = BTreeSet::from([equity.value.entity.as_str()]);
        for claim in &inputs.entity_valuations {
            if !valuation_entities.insert(claim.value.equity.entity.as_str()) {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "each secondary valuation must name a distinct entity different from the primary objective",
                ));
            }
            entity::valuation::validate(claim, policy, &states, &candidates)?;
            metadata.include(claim)?;
        }
    }
    Ok(metadata)
}

struct FundingIndex<'a> {
    declaration: &'a ProgramFunding,
    requirements: BTreeMap<(&'a str, &'a str), Money>,
}

impl<'a> FundingIndex<'a> {
    fn new(declaration: &'a ProgramFunding) -> Self {
        Self {
            declaration,
            requirements: declaration
                .requirements
                .iter()
                .map(|r| ((r.candidate_id.as_str(), r.state_id.as_str()), r.value))
                .collect(),
        }
    }

    fn assess(&self, selected: &[String]) -> Result<FundingAssessment> {
        let money = |major| ExactMoney {
            currency: self.declaration.currency,
            major,
        };
        let minimum_cash = amount(self.declaration.minimum_cash)?;
        let mut minimum_slack = None;
        let mut states = Vec::with_capacity(self.declaration.available_cash.len());
        for (state, available) in &self.declaration.available_cash {
            let mut required = 0_i128;
            for candidate in selected {
                let value = self
                    .requirements
                    .get(&(candidate.as_str(), state.as_str()))
                    .ok_or_else(|| {
                        bad(
                            RefusalCode::NumericalCertificateFailed207,
                            "admitted funding matrix lost a candidate/state entry",
                        )
                    })?;
                required = checked_minor_add(required, value.minor)?;
            }
            let cash_required = arithmetic(Rat::new(required, Money::SCALE))?;
            let available_cash = amount(*available)?;
            let remaining_cash = arithmetic(available_cash.checked_sub(cash_required))?;
            let slack = arithmetic(remaining_cash.checked_sub(minimum_cash))?;
            let improves = match minimum_slack {
                None => true,
                Some(old) => arithmetic(slack.checked_cmp(old))?.is_lt(),
            };
            if improves {
                minimum_slack = Some(slack);
            }
            states.push(FundingState {
                state_id: state.clone(),
                available_cash: money(available_cash),
                cash_required: money(cash_required),
                remaining_cash: money(remaining_cash),
                minimum_cash: money(minimum_cash),
                slack: money(slack),
            });
        }
        let minimum_slack = minimum_slack.ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "admitted funding has no shared states",
            )
        })?;
        Ok(FundingAssessment {
            states,
            minimum_slack: money(minimum_slack),
        })
    }
}

/// Admission and canonical sorting make these complete matrices dense in period/candidate/state order.
struct FundingScheduleIndex<'a> {
    declaration: &'a FundingSchedule,
    candidate_count: usize,
}

impl FundingScheduleIndex<'_> {
    fn assess(&self, selected_mask: usize) -> Result<FundingScheduleAssessment> {
        let schedule = self.declaration;
        let money = |minor| -> Result<ExactMoney> {
            Ok(ExactMoney {
                currency: schedule.currency,
                major: amount(Money::from_minor(schedule.currency, minor))?,
            })
        };
        let missing = || {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "admitted cash schedule lost a complete matrix entry",
            )
        };
        let first = schedule.periods.first().ok_or_else(missing)?;
        let state_count = schedule.opening_cash.len();
        let mut balances = Vec::with_capacity(state_count);
        let mut checkpoints = Vec::with_capacity(state_count * (schedule.periods.len() + 1));
        let mut minimum_slack: Option<i128> = None;
        for (state, opening) in &schedule.opening_cash {
            let slack = opening
                .minor
                .checked_sub(first.minimum_cash.minor)
                .ok_or_else(|| {
                    bad(
                        RefusalCode::NumericalCertificateFailed207,
                        "cash-floor difference overflowed",
                    )
                })?;
            minimum_slack = Some(minimum_slack.map_or(slack, |old| old.min(slack)));
            balances.push(opening.minor);
            checkpoints.push(CashScheduleCheckpoint {
                period_id: None,
                checkpoint_date: schedule.opening_date.clone(),
                state_id: state.clone(),
                opening_cash: money(opening.minor)?,
                baseline_cash_flow: money(0)?,
                candidate_cash_flow: money(0)?,
                closing_cash: money(opening.minor)?,
                minimum_cash: money(first.minimum_cash.minor)?,
                slack: money(slack)?,
            });
        }
        for (period_index, period) in schedule.periods.iter().enumerate() {
            let (_, closing_date) = period_dates(&period.cash_period).ok_or_else(missing)?;
            for (state_index, ((state, _), balance)) in
                schedule.opening_cash.iter().zip(&mut balances).enumerate()
            {
                let baseline = schedule
                    .baseline_flows
                    .get(period_index * state_count + state_index)
                    .ok_or_else(missing)?
                    .value
                    .minor;
                let mut program = 0_i128;
                for candidate_index in 0..self.candidate_count {
                    if selected_mask & (1 << candidate_index) == 0 {
                        continue;
                    }
                    let flow = schedule
                        .candidate_flows
                        .get(
                            (period_index * self.candidate_count + candidate_index) * state_count
                                + state_index,
                        )
                        .ok_or_else(missing)?;
                    program = checked_minor_add(program, flow.value.minor)?;
                }
                let opening = *balance;
                let closing = checked_minor_add(checked_minor_add(opening, baseline)?, program)?;
                let slack = closing
                    .checked_sub(period.minimum_cash.minor)
                    .ok_or_else(|| {
                        bad(
                            RefusalCode::NumericalCertificateFailed207,
                            "cash-floor difference overflowed",
                        )
                    })?;
                minimum_slack = Some(minimum_slack.map_or(slack, |old| old.min(slack)));
                checkpoints.push(CashScheduleCheckpoint {
                    period_id: Some(period.id.clone()),
                    checkpoint_date: closing_date.into(),
                    state_id: state.clone(),
                    opening_cash: money(opening)?,
                    baseline_cash_flow: money(baseline)?,
                    candidate_cash_flow: money(program)?,
                    closing_cash: money(closing)?,
                    minimum_cash: money(period.minimum_cash.minor)?,
                    slack: money(slack)?,
                });
                *balance = closing;
            }
        }
        Ok(FundingScheduleAssessment {
            checkpoints,
            minimum_slack: money(minimum_slack.ok_or_else(missing)?)?,
        })
    }
}

struct EquityIndex<'a> {
    declaration: &'a EquityBasis,
    nav_changes: Vec<Money>,
    candidate_count: usize,
    baseline_shares: Rat,
    baseline_per_share: Vec<Rat>,
}

impl<'a> EquityIndex<'a> {
    fn new(declaration: &'a EquityBasis, candidates: &[Claim<CapitalCandidate>]) -> Result<Self> {
        Self::with_nav_changes(
            declaration,
            candidates
                .iter()
                .flat_map(|claim| claim.value.payoffs.iter().map(|(_, value)| *value))
                .collect(),
        )
    }

    fn with_nav_changes(declaration: &'a EquityBasis, nav_changes: Vec<Money>) -> Result<Self> {
        let state_count = declaration.baseline_nav.len();
        if state_count == 0 || !nav_changes.len().is_multiple_of(state_count) {
            return Err(bad(
                RefusalCode::NumericalCertificateFailed207,
                "admitted equity NAV matrix lost its complete shared-state shape",
            ));
        }
        let candidate_count = nav_changes.len() / state_count;
        let baseline_shares = arithmetic(
            declaration
                .initial_share_units
                .checked_mul(declaration.shares_per_unit),
        )?;
        let baseline_per_share = declaration
            .baseline_nav
            .iter()
            .map(|(_, nav)| arithmetic(amount(*nav)?.checked_div(baseline_shares)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            declaration,
            nav_changes,
            candidate_count,
            baseline_shares,
            baseline_per_share,
        })
    }

    fn assess(&self, selected_mask: usize) -> Result<EquityAssessment> {
        let equity = self.declaration;
        let money = |major| ExactMoney {
            currency: equity.currency,
            major,
        };
        let per_share = |major| ExactPerShare {
            currency: equity.currency,
            major,
        };
        let baseline_shares = self.baseline_shares;
        let state_count = equity.baseline_nav.len();
        let missing = || {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "admitted equity matrix lost a candidate/state entry",
            )
        };
        let mut states = Vec::with_capacity(state_count);
        for (state_index, (state_id, baseline)) in equity.baseline_nav.iter().enumerate() {
            let mut nav_change_minor = 0_i128;
            let mut changed_units = ZERO;
            for candidate_index in 0..self.candidate_count {
                if selected_mask & (1 << candidate_index) == 0 {
                    continue;
                }
                let payoff = self
                    .nav_changes
                    .get(candidate_index * state_count + state_index)
                    .ok_or_else(missing)?;
                nav_change_minor = checked_minor_add(nav_change_minor, payoff.minor)?;
                let change = equity
                    .share_changes
                    .get(candidate_index * state_count + state_index)
                    .ok_or_else(missing)?;
                changed_units = arithmetic(changed_units.checked_add(change.units))?;
            }
            let baseline_nav = amount(*baseline)?;
            let nav_change = amount(Money::from_minor(equity.currency, nav_change_minor))?;
            let ending_nav = amount(Money::from_minor(
                equity.currency,
                checked_minor_add(baseline.minor, nav_change_minor)?,
            ))?;
            let share_change = arithmetic(changed_units.checked_mul(equity.shares_per_unit))?;
            let ending_units = arithmetic(equity.initial_share_units.checked_add(changed_units))?;
            let ending_shares = arithmetic(ending_units.checked_mul(equity.shares_per_unit))?;
            let baseline_nav_per_share = *self
                .baseline_per_share
                .get(state_index)
                .ok_or_else(missing)?;
            let (ending_nav_per_share, delta_nav_per_share) = if ending_shares.signum() > 0 {
                let ending = arithmetic(ending_nav.checked_div(ending_shares))?;
                let delta = arithmetic(ending.checked_sub(baseline_nav_per_share))?;
                (Some(per_share(ending)), Some(per_share(delta)))
            } else {
                (None, None)
            };
            states.push(EquityState {
                state_id: state_id.clone(),
                baseline_nav: money(baseline_nav),
                nav_change: money(nav_change),
                ending_nav: money(ending_nav),
                baseline_shares,
                share_change,
                ending_shares,
                baseline_nav_per_share: per_share(baseline_nav_per_share),
                ending_nav_per_share,
                delta_nav_per_share,
            });
        }
        Ok(EquityAssessment { states })
    }
}

struct EquityStatisticsInput<'a> {
    action_id: &'a str,
    label: String,
    role: ActionRole,
    assessment: &'a EquityAssessment,
}

impl<'a> EquityStatisticsInput<'a> {
    fn new(choice: &'a ProgramChoice, assessment: &'a EquityAssessment) -> Self {
        Self {
            action_id: &choice.bundle_id,
            label: if choice.selected.is_empty() {
                "Do nothing".into()
            } else {
                choice.selected.join(" + ")
            },
            role: if choice.selected.is_empty() {
                ActionRole::DoNothing
            } else {
                ActionRole::Action
            },
            assessment,
        }
    }
}

/// One shared calculation for every entity; the caller supplies the joint pre-risk feasible population.
fn equity_statistics(
    population: &[&EquityStatisticsInput<'_>],
    weights: &WeightSet,
    currency: Currency,
    tail_mass: Rat,
) -> Result<Vec<(PerShareStats, ExactPerShare)>> {
    let missing = || {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "structurally feasible equity program lost a defined shared-state objective",
        )
    };
    let mut best = vec![None; weights.states.len()];
    for input in population {
        if input.assessment.states.len() != weights.states.len() {
            return Err(missing());
        }
        for (index, state) in input.assessment.states.iter().enumerate() {
            let value = state.delta_nav_per_share.ok_or_else(missing)?.major;
            let old = best.get_mut(index).ok_or_else(missing)?;
            if match *old {
                None => true,
                Some(previous) => exact_compare(value, previous).is_gt(),
            } {
                *old = Some(value);
            }
        }
    }
    let per_share = |major| ExactPerShare { currency, major };
    let mut outcomes = Vec::with_capacity(population.len());
    for input in population {
        let assessment = input.assessment;
        let mut values = Vec::with_capacity(weights.states.len());
        let mut states = Vec::with_capacity(weights.states.len());
        let mut mean_regret = ZERO;
        let mut max_regret = ZERO;
        for (index, state) in assessment.states.iter().enumerate() {
            let (state_id, weight) = weights.states.get(index).ok_or_else(missing)?;
            if state_id != &state.state_id {
                return Err(missing());
            }
            let value = state.delta_nav_per_share.ok_or_else(missing)?.major;
            let optimum = best.get(index).copied().flatten().ok_or_else(missing)?;
            let regret = arithmetic(optimum.checked_sub(value))?;
            mean_regret =
                arithmetic(mean_regret.checked_add(arithmetic(regret.checked_mul(*weight))?))?;
            if exact_compare(regret, max_regret).is_gt() {
                max_regret = regret;
            }
            values.push((value, *weight));
            states.push(PerShareStateResult {
                state_id: state_id.clone(),
                weight: *weight,
                payoff: per_share(value),
                best_in_hindsight: per_share(optimum),
                regret: per_share(regret),
            });
        }
        let distribution = ExactDistribution::new(&values)?;
        let tail = distribution.tail_loss(tail_mass)?;
        let statistics = PerShareStats {
            action_id: input.action_id.into(),
            label: input.label.clone(),
            role: input.role,
            mean: per_share(distribution.mean()),
            p10_p50_p90: [
                per_share(distribution.quantile(arithmetic(Rat::new(1, 10))?)?),
                per_share(distribution.quantile(arithmetic(Rat::new(1, 2))?)?),
                per_share(distribution.quantile(arithmetic(Rat::new(9, 10))?)?),
            ],
            cvar95_loss: per_share(distribution.tail_loss(arithmetic(Rat::new(1, 20))?)?),
            mean_regret: per_share(mean_regret),
            max_regret: per_share(max_regret),
            states,
        };
        outcomes.push((statistics, per_share(tail)));
    }
    Ok(outcomes)
}

/// Per-share regret compares only subsets that satisfy every structural constraint in every state.
fn per_share_statistics(
    choices: &mut [ProgramChoice],
    weights: &WeightSet,
    equity: &EquityBasis,
    tail_mass: Rat,
) -> Result<()> {
    let missing = || {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "structurally feasible equity program lost its primary assessment",
        )
    };
    let population = choices
        .iter()
        .enumerate()
        .filter(|(_, choice)| choice.structural_rejections.is_empty())
        .map(|(index, choice)| {
            Ok((
                index,
                EquityStatisticsInput::new(choice, choice.equity.as_ref().ok_or_else(missing)?),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let outcomes = equity_statistics(
        &population
            .iter()
            .map(|(_, input)| input)
            .collect::<Vec<_>>(),
        weights,
        equity.currency,
        tail_mass,
    )?;
    let indices: Vec<_> = population.iter().map(|(index, _)| *index).collect();
    drop(population);
    for (index, (statistics, tail)) in indices.into_iter().zip(outcomes) {
        let choice = choices.get_mut(index).ok_or_else(missing)?;
        choice.per_share_statistics = Some(statistics);
        choice.per_share_tail_loss = Some(tail);
        choice.risk_rejected = match equity.max_tail_loss_per_share {
            Some(limit) => exact_compare(tail.major, limit).is_gt(),
            None => false,
        };
    }
    Ok(())
}

/// Exact upper-tail signed loss from the complete joint payoff vector, including fractional boundary mass.
fn tail_loss(action: &ActionPayoffs, weights: &WeightSet, tail: Rat) -> Result<Rat> {
    let weights: BTreeMap<_, _> = weights
        .states
        .iter()
        .map(|(id, w)| (id.as_str(), *w))
        .collect();
    let mut values = action.values.clone();
    values.sort_by(|a, b| a.1.minor.cmp(&b.1.minor).then(a.0.cmp(&b.0)));
    let (mut remaining, mut sum) = (tail, ZERO);
    for (state, value) in values {
        if remaining == ZERO {
            break;
        }
        let weight = weights.get(state.as_str()).ok_or_else(|| {
            bad(
                RefusalCode::NumericalCertificateFailed207,
                "missing admitted state weight",
            )
        })?;
        let take = if arithmetic(weight.checked_cmp(remaining))?.is_gt() {
            remaining
        } else {
            *weight
        };
        sum = arithmetic(sum.checked_add(arithmetic(amount(value)?.checked_mul(take))?))?;
        remaining = arithmetic(remaining.checked_sub(take))?;
    }
    if remaining != ZERO {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "tail mass was not covered",
        ));
    }
    arithmetic(arithmetic(sum.checked_div(tail))?.checked_neg())
}

fn winners(
    choices: &[ProgramChoice],
    risk: bool,
    per_share: bool,
) -> Result<(Vec<String>, Option<Rat>)> {
    let mut best = None;
    let mut ids = Vec::new();
    for row in choices {
        if risk && row.risk_rejected {
            continue;
        }
        let mean = if per_share {
            row.per_share_statistics
                .as_ref()
                .map(|stats| stats.mean.major)
        } else {
            row.statistics.as_ref().map(|stats| stats.mean.major)
        };
        let Some(mean) = mean else {
            continue;
        };
        let cmp = match best {
            None => std::cmp::Ordering::Greater,
            Some(old) if per_share => exact_compare(mean, old),
            Some(old) => arithmetic(mean.checked_cmp(old))?,
        };
        if cmp.is_gt() {
            best = Some(mean);
            ids.clear();
        }
        if !cmp.is_lt() {
            ids.push(row.bundle_id.clone());
        }
    }
    Ok((ids, best))
}

/// Shared complete-population statistics for static bundles and nonanticipative policies.
fn populate_statistics(
    choices: &mut [ProgramChoice],
    actions: &[Claim<ActionPayoffs>],
    inputs: &ProgramInputs,
) -> Result<()> {
    let policy = &inputs.policy.value;
    let money = |major| ExactMoney {
        currency: policy.currency,
        major,
    };
    let stats = if actions.is_empty() {
        // No admitted choice means no invented hold-cash statistics or counterfactual regret baseline.
        Vec::new()
    } else {
        card::evaluate_feasible(&DecisionInputs {
            weights: inputs.weights.clone(),
            actions: actions.to_vec(),
            information_options: vec![],
            robustness: None,
            downside_limit: None,
        })?
        .summary()
        .alternatives
        .clone()
    };
    let mut statistics: BTreeMap<_, _> = stats
        .into_iter()
        .map(|s| (s.action_id.clone(), s))
        .collect();
    let mut tails = BTreeMap::new();
    for action in actions {
        tails.insert(
            action.value.action_id.clone(),
            tail_loss(&action.value, &inputs.weights.value, policy.tail_mass)?,
        );
    }
    for row in choices.iter_mut() {
        if let Some(stats) = statistics.remove(&row.bundle_id) {
            let tail = tails.remove(&row.bundle_id).ok_or_else(|| {
                bad(
                    RefusalCode::NumericalCertificateFailed207,
                    "missing computed tail",
                )
            })?;
            row.risk_rejected = match policy.max_tail_loss {
                Some(cap) => arithmetic(tail.checked_cmp(amount(cap)?))?.is_gt(),
                None => false,
            };
            row.tail_loss = Some(money(tail));
            row.statistics = Some(stats);
        }
    }
    if let Some(equity) = &inputs.equity {
        per_share_statistics(
            choices,
            &inputs.weights.value,
            &equity.value,
            policy.tail_mass,
        )?;
    }
    entity::valuation::populate_statistics(choices, inputs)?;
    Ok(())
}

/// Enumerates all declared binary subsets; infeasibility remains in the returned population.
pub fn evaluate(inputs: &ProgramInputs) -> Result<EvaluatedProgram> {
    let metadata = validate(inputs)?;
    let mut inputs = inputs.clone();
    inputs
        .candidates
        .sort_by(|a, b| a.value.candidate_id.cmp(&b.value.candidate_id));
    inputs.weights.value.states.sort_by(|a, b| a.0.cmp(&b.0));
    for candidate in &mut inputs.candidates {
        candidate.value.payoffs.sort_by(|a, b| a.0.cmp(&b.0));
        candidate.value.requires.sort();
    }
    if let Some(funding) = &mut inputs.funding {
        funding.value.available_cash.sort_by(|a, b| a.0.cmp(&b.0));
        funding
            .value
            .requirements
            .sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    }
    if let Some(claim) = &mut inputs.funding_schedule {
        let schedule = &mut claim.value;
        schedule
            .periods
            .sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
        schedule.opening_cash.sort_by(|a, b| a.0.cmp(&b.0));
        let periods: BTreeMap<_, _> = schedule
            .periods
            .iter()
            .enumerate()
            .map(|(index, period)| (period.id.as_str(), index))
            .collect();
        schedule.baseline_flows.sort_by(|a, b| {
            (periods.get(a.period_id.as_str()), &a.state_id)
                .cmp(&(periods.get(b.period_id.as_str()), &b.state_id))
        });
        schedule.candidate_flows.sort_by(|a, b| {
            (
                periods.get(a.period_id.as_str()),
                &a.candidate_id,
                &a.state_id,
            )
                .cmp(&(
                    periods.get(b.period_id.as_str()),
                    &b.candidate_id,
                    &b.state_id,
                ))
        });
    }
    if let Some(claim) = &mut inputs.equity {
        claim.value.baseline_nav.sort_by(|a, b| a.0.cmp(&b.0));
        claim
            .value
            .share_changes
            .sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    }
    if let Some(claim) = &mut inputs.entity_funding {
        entity::canonicalize(&mut claim.value);
    }
    inputs
        .entity_valuations
        .sort_by(|a, b| a.value.equity.entity.cmp(&b.value.equity.entity));
    for claim in &mut inputs.entity_valuations {
        entity::valuation::canonicalize(&mut claim.value);
    }
    let funding_index = inputs
        .funding
        .as_ref()
        .map(|claim| FundingIndex::new(&claim.value));
    let schedule_index = inputs
        .funding_schedule
        .as_ref()
        .map(|claim| FundingScheduleIndex {
            declaration: &claim.value,
            candidate_count: inputs.candidates.len(),
        });
    let equity_index = inputs
        .equity
        .as_ref()
        .map(|claim| EquityIndex::new(&claim.value, &inputs.candidates))
        .transpose()?;
    let entity_index = inputs
        .entity_funding
        .as_ref()
        .map(|claim| entity::Index::new(&claim.value, &inputs.candidates, &inputs.weights.value))
        .transpose()?;
    let valuation_index = entity::valuation::Index::new(&inputs.entity_valuations)?;
    let policy = &inputs.policy.value;
    let money = |major| ExactMoney {
        currency: policy.currency,
        major,
    };
    let mut choices = Vec::new();
    let mut actions = Vec::new();
    for mask in 0..(1_usize << inputs.candidates.len()) {
        let selected: Vec<_> = inputs
            .candidates
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, c)| &c.value)
            .collect();
        let ids: Vec<_> = selected.iter().map(|c| c.candidate_id.clone()).collect();
        let spend = selected
            .iter()
            .try_fold(0_i128, |sum, c| checked_minor_add(sum, c.cost.minor))?;
        let mut rejected = Vec::new();
        if spend > policy.budget.minor {
            rejected.push(StructuralRejection::BudgetExceeded);
        }
        let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for c in &selected {
            for required in &c.requires {
                if !ids.contains(required) {
                    rejected.push(StructuralRejection::MissingPrerequisite {
                        candidate: c.candidate_id.clone(),
                        prerequisite: required.clone(),
                    });
                }
            }
            if let Some(group) = &c.exclusion_group {
                groups
                    .entry(group)
                    .or_default()
                    .push(c.candidate_id.clone());
            }
        }
        for (group, candidates) in groups {
            if candidates.len() > 1 {
                rejected.push(StructuralRejection::MutualExclusion {
                    group: group.into(),
                    candidates,
                });
            }
        }
        let funding = funding_index
            .as_ref()
            .map(|index| index.assess(&ids))
            .transpose()?;
        if let Some(funding) = &funding {
            for state in &funding.states {
                if state.slack.major.signum() < 0 {
                    rejected.push(StructuralRejection::LiquidityShortfall {
                        state_id: state.state_id.clone(),
                        shortfall: money(arithmetic(state.slack.major.checked_neg())?),
                    });
                }
            }
        }
        let funding_schedule = schedule_index
            .as_ref()
            .map(|index| index.assess(mask))
            .transpose()?;
        if let Some(schedule) = &funding_schedule {
            for checkpoint in &schedule.checkpoints {
                if checkpoint.slack.major.signum() < 0 {
                    rejected.push(StructuralRejection::CashScheduleShortfall {
                        period_id: checkpoint.period_id.clone(),
                        checkpoint_date: checkpoint.checkpoint_date.clone(),
                        state_id: checkpoint.state_id.clone(),
                        shortfall: money(arithmetic(checkpoint.slack.major.checked_neg())?),
                    });
                }
            }
        }
        let entity_funding = entity_index
            .as_ref()
            .map(|index| index.assess(mask))
            .transpose()?;
        if let Some(funding) = &entity_funding {
            for state in &funding.states {
                for checkpoint in &state.checkpoints {
                    if checkpoint.slack.major.signum() < 0 {
                        rejected.push(StructuralRejection::EntityCashShortfall {
                            entity_id: checkpoint.entity_id.clone(),
                            state_id: state.state_id.clone(),
                            period_id: checkpoint.period_id.clone(),
                            checkpoint_date: checkpoint.checkpoint_date.clone(),
                            phase: checkpoint.phase,
                            shortfall: money(arithmetic(checkpoint.slack.major.checked_neg())?),
                        });
                    }
                }
                for call in &state.capital_calls {
                    if call.funding_surplus.major.signum() < 0 {
                        rejected.push(StructuralRejection::EntityCapitalCallShortfall {
                            entity_id: call.entity_id.clone(),
                            call_id: call.call_id.clone(),
                            state_id: state.state_id.clone(),
                            period_id: call.period_id.clone(),
                            checkpoint_date: call.checkpoint_date.clone(),
                            shortfall: money(arithmetic(call.funding_surplus.major.checked_neg())?),
                        });
                    }
                }
            }
        }
        let equity = equity_index
            .as_ref()
            .map(|index| index.assess(mask))
            .transpose()?;
        if let Some(equity) = &equity {
            for state in &equity.states {
                if state.ending_shares.signum() <= 0 {
                    rejected.push(StructuralRejection::NonpositiveShareCount {
                        state_id: state.state_id.clone(),
                        shares: state.ending_shares,
                    });
                }
            }
        }
        let entity_valuations = valuation_index.assess(mask)?;
        for valuation in &entity_valuations {
            for state in &valuation.equity.states {
                if state.ending_shares.signum() <= 0 {
                    rejected.push(StructuralRejection::EntityNonpositiveShareCount {
                        entity: valuation.entity.clone(),
                        share_class: valuation.share_class.clone(),
                        state_id: state.state_id.clone(),
                        shares: state.ending_shares,
                    });
                }
            }
        }
        let bundle_id = format!("bundle_{mask:02x}");
        if rejected.is_empty() {
            let values = inputs
                .weights
                .value
                .states
                .iter()
                .enumerate()
                .map(|(i, (state, _))| {
                    let minor = selected.iter().try_fold(0_i128, |sum, c| {
                        checked_minor_add(sum, c.payoffs[i].1.minor)
                    })?;
                    Ok((state.clone(), Money::from_minor(policy.currency, minor)))
                })
                .collect::<Result<Vec<_>>>()?;
            let action = ActionPayoffs {
                model_key: policy.model_key.clone(),
                action_id: bundle_id.clone(),
                label: if ids.is_empty() {
                    "Do nothing".into()
                } else {
                    ids.join(" + ")
                },
                role: if ids.is_empty() {
                    ActionRole::DoNothing
                } else {
                    ActionRole::Action
                },
                currency: policy.currency,
                valuation_basis: policy.valuation_basis.clone(),
                values,
            };
            let tag = action.canonical_tag();
            actions.push(metadata.claim(
                "capital.bundle.net-delta-nav/1",
                action,
                &tag,
                AdditivityClass::ExtensiveAdditive,
                true,
            ));
        }
        let spend = arithmetic(Rat::new(spend, Money::SCALE))?;
        choices.push(ProgramChoice {
            bundle_id,
            selected: ids,
            spend: money(spend),
            budget_slack: money(arithmetic(amount(policy.budget)?.checked_sub(spend))?),
            funding,
            funding_schedule,
            entity_funding,
            equity,
            entity_valuations,
            structural_rejections: rejected,
            statistics: None,
            tail_loss: None,
            per_share_statistics: None,
            per_share_tail_loss: None,
            risk_rejected: false,
        });
    }
    populate_statistics(&mut choices, &actions, &inputs)?;
    let per_share_mode = inputs.equity.is_some();
    let (unconstrained_winners, before) = winners(&choices, false, per_share_mode)?;
    let (feasible_winners, after) = winners(&choices, true, per_share_mode)?;
    let (risk_cost, per_share_risk_cost) = match (before, after) {
        (Some(a), Some(b)) => {
            let difference = arithmetic(a.checked_sub(b))?;
            if per_share_mode {
                (
                    None,
                    Some(ExactPerShare {
                        currency: policy.currency,
                        major: difference,
                    }),
                )
            } else {
                (Some(money(difference)), None)
            }
        }
        _ => (None, None),
    };
    let selection_stability =
        stability::assess(&inputs, &choices, None, &metadata, &feasible_winners)?;
    let summary = ProgramSummary {
        model_key: policy.model_key.clone(),
        question: inputs.weights.value.question.clone(),
        policy: policy.clone(),
        funding: inputs.funding.as_ref().map(|claim| claim.value.clone()),
        funding_schedule: inputs
            .funding_schedule
            .as_ref()
            .map(|claim| claim.value.clone()),
        entity_funding: inputs
            .entity_funding
            .as_ref()
            .map(|claim| claim.value.clone()),
        equity: inputs.equity.as_ref().map(|claim| claim.value.clone()),
        entity_valuations: inputs
            .entity_valuations
            .iter()
            .map(|claim| claim.value.clone())
            .collect(),
        choices,
        unconstrained_winners,
        feasible_winners,
        expected_value_cost_of_risk_limit: risk_cost,
        expected_per_share_cost_of_risk_limit: per_share_risk_cost,
        selection_stability,
    };
    let mut binding = Encoder::new_frame(*b"CAPP", 1);
    binding.put_str(&render::inputs_json(&inputs));
    binding.put_str(&render::summary_json(&summary));
    binding.put_str(no_claim_for(&inputs));
    let digest = sha256(&binding.finish()).to_hex();
    let tag = format!("capital-program/v1:{digest}");
    let claim = metadata.claim(
        "capital.static-program/1",
        summary,
        &tag,
        AdditivityClass::NonAdditive,
        false,
    );
    Ok(EvaluatedProgram {
        inputs,
        claim,
        digest,
    })
}

#[cfg(test)]
mod tests;
