//! Determinism: BitwisePortable
//!
//! Bitemporal capital-approval hindcast engine (`BD-230`, `GATE-CAP-04`, plan §23.1 & §23.9):
//! reconstructs each past capital approval strictly `as_of_decision(t_approval)` (`INV-016`,
//! `G-TIME-2`, `FMINE-REF-108`), compares `H7a` (static NPV/IRR + hurdle) against `H7d`
//! (per-share two-stage stochastic `CapitalProgram` with liquidity floor, exact sorted-tail
//! `CVaR` limit, and stage-gate recourse), and scores both frozen decisions against ex-post
//! realized outcomes (`INV-008`).
//!
//! No floating-point (`f64`) crosses any public boundary (`L-6`). Recourse option value is always
//! reported separately from static expected NPV (`G-CAP-D`).

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{EvidenceClass, Refusal, RefusalCode};
use fmine_time::{
    Anchor, AsOfQuery, CivilDate, NS_PER_DAY, QuadTemporalStamp, QueryPurpose, TimeError,
    TimeEvidence, ValidTime,
};
use fmine_units::ExactQty;

/// Mandatory no-claim boundary (`INV-002`).
pub const HINDCAST_NO_CLAIM: &str = "Decision-rule comparison under declared approval-time snapshots and supplied outcome payoffs. Payoffs for unchosen actions are counterfactual inputs. Scenario calibration, historical registration, investment authorization and regulatory sign-off require separate evidence.";

/// Minimum number of past approval cases required by `GATE-CAP-04` (plan §23.9).
pub const GATE_CAP_04_MIN_CASES: usize = 5;

const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);

/// Capital decision action compared between `H7a` and `H7d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ApprovalAction {
    ApproveFull,
    StageWithRecourse,
    DeferOrPass,
}

impl ApprovalAction {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApproveFull => "approve_full",
            Self::StageWithRecourse => "stage_with_recourse",
            Self::DeferOrPass => "defer_or_pass",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::ApproveFull => 1,
            Self::StageWithRecourse => 2,
            Self::DeferOrPass => 3,
        }
    }
}

/// One time-stamped observation admitted into the approval-time information set (`INV-016`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalEvidence {
    pub evidence_id: String,
    pub label: String,
    pub evidence_class: EvidenceClass,
    pub stamp: QuadTemporalStamp,
}

/// One shared macro/overrun/regime scenario available at `t_approval` (all values in exact USD millions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HindcastScenario {
    pub scenario_id: String,
    /// Strictly positive scenario probability weight; weights across scenarios must sum to `1`.
    pub weight: ExactQty,
    /// Net ΔNAV (USD millions) under immediate full-project commitment (already net of capex).
    pub full_commit_delta_nav_usd_m: ExactQty,
    /// Net ΔNAV (USD millions) under staged gate + recourse (study/FEED -> continue, rescale, or abandon).
    pub staged_recourse_delta_nav_usd_m: ExactQty,
    /// Net ΔNAV (USD millions) under defer/pass (e.g. `0` or benchmark alternative).
    pub defer_delta_nav_usd_m: ExactQty,
    /// Minimum liquidity slack above board cash floor (USD millions) under full commitment (`< 0` fails).
    pub full_commit_liquidity_slack_usd_m: ExactQty,
    /// Minimum liquidity slack above board cash floor (USD millions) under staged recourse (`< 0` fails).
    pub staged_recourse_liquidity_slack_usd_m: ExactQty,
    /// Minimum liquidity slack above board cash floor (USD millions) under defer/pass (`< 0` fails).
    pub defer_liquidity_slack_usd_m: ExactQty,
}

/// Realized ex-post outcome observed strictly after `approval_as_of_ns` (`INV-008`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedApprovalOutcome {
    /// UTC date of the end of the outcome's physical support.
    pub realized_date: CivilDate,
    pub realized_stamp: QuadTemporalStamp,
    /// Realized capital expenditure (USD millions, `> 0`).
    pub realized_capex_usd_m: ExactQty,
    /// Realized schedule slip in months (`> 0` means late, `< 0` means early).
    pub schedule_slip_months: i32,
    /// Supplied net ΔNAV (USD millions) for full commitment, counterfactual if unchosen.
    pub realized_full_commit_delta_nav_usd_m: ExactQty,
    /// Supplied net ΔNAV (USD millions) for staged recourse, counterfactual if unchosen.
    pub realized_staged_recourse_delta_nav_usd_m: ExactQty,
    /// Supplied net ΔNAV (USD millions) for defer/pass, counterfactual if unchosen.
    pub realized_defer_delta_nav_usd_m: ExactQty,
    /// Citation IDs or source notes documenting the realized outcome.
    pub outcome_evidence_ids: Vec<String>,
}

/// Complete bitemporal approval hindcast case (`as_of_decision` snapshot + ex-post realized outcome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalCase<T = RealizedApprovalOutcome> {
    pub case_id: String,
    pub project_name: String,
    /// UTC civil date of the approval cut.
    pub approval_date: CivilDate,
    /// Knowledge cut-off timestamp (`as_of_decision(t_approval)`, UTC ns).
    pub approval_as_of_ns: i64,
    /// Ledger commit anchor at `approval_as_of_ns` (`INV-012`).
    pub approval_anchor: Anchor,
    /// Physical period covered by approval-time observations.
    pub physical_horizon: ValidTime,
    /// Explicitly tags whether the approval-time scenario payoffs are synthetic/assumed.
    pub is_synthetic_scenario_grid: bool,
    /// Approval-time observations verified strictly admissible under `AsOfQuery::OperationalDecision`.
    pub approval_evidence: Vec<ApprovalEvidence>,
    /// Baseline approved capital estimate (USD millions, `> 0`).
    pub baseline_capex_usd_m: ExactQty,
    /// Share count denominator (millions of shares, `> 0`) for per-share `ΔNAV/share`.
    pub shares_millions: ExactQty,
    /// Static point-estimate NPV presented to `H7a` (USD millions).
    pub h7a_static_npv_usd_m: ExactQty,
    /// Static point-estimate IRR presented to `H7a` (exact fraction, e.g., `18/100` = 18%).
    pub h7a_static_irr: ExactQty,
    /// Board static hurdle IRR used by `H7a` (exact fraction, e.g., `15/100` = 15%).
    pub h7a_hurdle_irr: ExactQty,
    /// Shared approval-time scenario grid evaluated by `H7d`.
    pub scenarios: Vec<HindcastScenario>,
    /// Upper tail mass `α ∈ (0, 1]` for signed-loss `CVaR_α(-ΔNAV/share)` (e.g. `1/5` = worst 20%).
    pub cvar_tail_mass: ExactQty,
    /// Maximum admissible `CVaR_α` loss per share (USD/share, `>= 0`).
    pub max_cvar_loss_per_share_usd: ExactQty,
    /// Ex-post outcome input, or unit for approval-only callers.
    pub realized_outcome: T,
}

/// Inputs available for a decision before any outcome is supplied.
pub type ApprovalSnapshot = ApprovalCase<()>;

/// Exact per-share metrics for one action under `H7d` at `t_approval`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionApprovalMetrics {
    pub action: ApprovalAction,
    pub expected_delta_nav_per_share_usd: ExactQty,
    /// Signed-loss `CVaR_α(-ΔNAV/share)` in USD/share (positive = expected tail loss).
    pub cvar_loss_per_share_usd: ExactQty,
    pub min_liquidity_slack_usd_m: ExactQty,
    pub liquidity_feasible: bool,
    pub cvar_feasible: bool,
    pub feasible: bool,
}

/// The two selections and their immutable approval input identity, reusable when scoring later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenApproval {
    case_id: String,
    project_name: String,
    approval_date: CivilDate,
    approval_as_of_ns: i64,
    is_synthetic_scenario_grid: bool,
    admitted_evidence_ids: Vec<String>,
    digest: Digest32,
    h7a_decision: ApprovalAction,
    h7d_decision: ApprovalAction,
    action_metrics: [ActionApprovalMetrics; 3],
    recourse_option_value: ExactQty,
    baseline_capex_usd_m: ExactQty,
    shares_millions: ExactQty,
    full_commit_nav_band: [ExactQty; 2],
}

impl FrozenApproval {
    /// Approval input identity, independent of any subsequent outcome.
    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    /// Choices in H7a, H7d order.
    #[must_use]
    pub const fn decisions(&self) -> (ApprovalAction, ApprovalAction) {
        (self.h7a_decision, self.h7d_decision)
    }

    /// Exact approval-time results for full commitment, staged recourse, and defer/pass.
    #[must_use]
    pub const fn action_metrics(&self) -> &[ActionApprovalMetrics; 3] {
        &self.action_metrics
    }

    /// The availability cut shared by both rules.
    #[must_use]
    pub const fn approval_as_of_ns(&self) -> i64 {
        self.approval_as_of_ns
    }

    /// Whether the submitted approval scenarios were synthetic.
    #[must_use]
    pub const fn is_synthetic_scenario_grid(&self) -> bool {
        self.is_synthetic_scenario_grid
    }

    /// The source identifiers admitted at the cut, in canonical order.
    #[must_use]
    pub fn admitted_evidence_ids(&self) -> &[String] {
        &self.admitted_evidence_ids
    }
}

/// Scores the two rules on declared comparisons, including counterfactual payoffs for unchosen actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HindcastCaseResult {
    pub case_id: String,
    pub project_name: String,
    pub approval_date: CivilDate,
    pub is_synthetic_scenario_grid: bool,
    pub admitted_evidence_ids: Vec<String>,
    pub outcome_evidence_ids: Vec<String>,
    /// Approval inputs and selections, frozen before outcome scoring.
    pub approval_digest: Digest32,
    /// Supplied outcome and its dated evidence identifiers.
    pub outcome_digest: Digest32,
    pub h7a_decision: ApprovalAction,
    pub h7d_decision: ApprovalAction,
    pub decisions_diverged: bool,
    /// Separate static expected ΔNAV/share (`ApproveFull`) vs recourse option value (`G-CAP-D`).
    pub static_expected_delta_nav_per_share_usd: ExactQty,
    pub recourse_option_value_per_share_usd: ExactQty,
    pub action_metrics: [ActionApprovalMetrics; 3],
    pub realized_capex_multiple: ExactQty,
    pub schedule_slip_months: i32,
    pub h7a_realized_delta_nav_per_share_usd: ExactQty,
    pub h7d_realized_delta_nav_per_share_usd: ExactQty,
    pub best_hindsight_delta_nav_per_share_usd: ExactQty,
    pub h7a_regret_per_share_usd: ExactQty,
    pub h7d_regret_per_share_usd: ExactQty,
    pub h7d_advantage_per_share_usd: ExactQty,
    /// Whether the realized full-commit outcome fell within the approval-time `[min, max]` scenario band.
    pub realized_within_approval_band: bool,
}

/// Aggregate scorecard across bitemporal hindcasts (`BD-230`, `GATE-CAP-04`).
/// The sums are equal-case-weight descriptive scores across the declared cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HindcastScorecard {
    pub results: Vec<HindcastCaseResult>,
    pub cases_count: usize,
    pub synthetic_scenario_cases_count: usize,
    pub divergent_decisions_count: usize,
    pub band_covered_count: usize,
    pub h7a_total_realized_delta_nav_per_share_usd: ExactQty,
    pub h7d_total_realized_delta_nav_per_share_usd: ExactQty,
    pub h7a_total_regret_per_share_usd: ExactQty,
    pub h7d_total_regret_per_share_usd: ExactQty,
    pub h7d_net_advantage_per_share_usd: ExactQty,
    pub no_claim: &'static str,
    pub digest: Digest32,
}

/// Evaluate one or more bitemporal approval hindcast cases.
pub fn evaluate_hindcast_suite(cases: &[ApprovalCase]) -> Result<HindcastScorecard, Refusal> {
    if cases.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "bitemporal hindcast requires at least one ApprovalCase",
        ));
    }
    let mut seen_ids = BTreeSet::new();
    let mut results = Vec::with_capacity(cases.len());
    let mut synthetic_count = 0usize;
    let mut divergent_count = 0usize;
    let mut band_covered_count = 0usize;
    let mut h7a_total_realized = ZERO;
    let mut h7d_total_realized = ZERO;
    let mut h7a_total_regret = ZERO;
    let mut h7d_total_regret = ZERO;

    let mut ordered_cases: Vec<_> = cases.iter().collect();
    ordered_cases.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    for case in ordered_cases {
        if !seen_ids.insert(case.case_id.as_str()) {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("duplicate hindcast case_id {:?}", case.case_id),
            ));
        }
        let res = freeze_approval(case)?.score(&case.realized_outcome)?;
        if res.is_synthetic_scenario_grid {
            synthetic_count += 1;
        }
        if res.decisions_diverged {
            divergent_count += 1;
        }
        if res.realized_within_approval_band {
            band_covered_count += 1;
        }
        h7a_total_realized =
            checked_add(h7a_total_realized, res.h7a_realized_delta_nav_per_share_usd)?;
        h7d_total_realized =
            checked_add(h7d_total_realized, res.h7d_realized_delta_nav_per_share_usd)?;
        h7a_total_regret = checked_add(h7a_total_regret, res.h7a_regret_per_share_usd)?;
        h7d_total_regret = checked_add(h7d_total_regret, res.h7d_regret_per_share_usd)?;
        results.push(res);
    }

    let h7d_net_advantage = checked_sub(h7d_total_realized, h7a_total_realized)?;
    let digest = compute_scorecard_digest(
        &results,
        synthetic_count,
        divergent_count,
        band_covered_count,
        h7a_total_realized,
        h7d_total_realized,
        h7a_total_regret,
        h7d_total_regret,
        h7d_net_advantage,
    );

    Ok(HindcastScorecard {
        cases_count: results.len(),
        synthetic_scenario_cases_count: synthetic_count,
        divergent_decisions_count: divergent_count,
        band_covered_count,
        h7a_total_realized_delta_nav_per_share_usd: h7a_total_realized,
        h7d_total_realized_delta_nav_per_share_usd: h7d_total_realized,
        h7a_total_regret_per_share_usd: h7a_total_regret,
        h7d_total_regret_per_share_usd: h7d_total_regret,
        h7d_net_advantage_per_share_usd: h7d_net_advantage,
        no_claim: HINDCAST_NO_CLAIM,
        digest,
        results,
    })
}

/// Evaluate a `GATE-CAP-04` bitemporal hindcast requiring at least [`GATE_CAP_04_MIN_CASES`] (`>= 5`)
/// cases. Gate qualification also requires audited historical inputs and independent review.
pub fn evaluate_gate_cap_04_hindcast(cases: &[ApprovalCase]) -> Result<HindcastScorecard, Refusal> {
    if cases.len() < GATE_CAP_04_MIN_CASES {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!(
                "GATE-CAP-04 requires at least {} past approvals, got {}",
                GATE_CAP_04_MIN_CASES,
                cases.len()
            ),
        ));
    }
    evaluate_hindcast_suite(cases)
}

/// Exact sorted-tail fractional-mass `CVaR_α(loss)` over finite weighted scenarios (`PO-PUSH-8`).
///
/// `weighted_losses` holds `(weight, loss)` pairs with `weight > 0` and `sum(weight) == 1`.
/// Sorts scenarios by descending `loss`, accumulates exact tail mass up to `alpha ∈ (0, 1]`,
/// splits the boundary atom proportionally, and returns `tail_weighted_loss / alpha`.
pub fn exact_weighted_cvar(
    weighted_losses: &[(ExactQty, ExactQty)],
    alpha: ExactQty,
) -> Result<ExactQty, Refusal> {
    if weighted_losses.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "CVaR requires at least one scenario",
        ));
    }
    if alpha.signum() <= 0 || cmp_qty(alpha, ONE)? == Ordering::Greater {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "CVaR tail mass alpha must lie in (0, 1], got {}/{}",
                alpha.num(),
                alpha.den()
            ),
        ));
    }

    let mut weight_sum = ZERO;
    let mut items: Vec<(ExactQty, ExactQty)> = Vec::with_capacity(weighted_losses.len());
    for &(w, loss) in weighted_losses {
        if w.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "CVaR scenario weights must be strictly positive",
            ));
        }
        weight_sum = checked_add(weight_sum, w)?;
        items.push((w, loss));
    }
    if weight_sum != ONE {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "CVaR scenario weights must sum to 1, got {}/{}",
                weight_sum.num(),
                weight_sum.den()
            ),
        ));
    }

    // Sort by descending loss (worst losses first) using exact checked comparison.
    for i in 1..items.len() {
        let mut j = i;
        while j > 0 && cmp_qty(items[j].1, items[j - 1].1)? == Ordering::Greater {
            items.swap(j, j - 1);
            j -= 1;
        }
    }

    let mut remaining = alpha;
    let mut weighted_tail = ZERO;
    for (w, loss) in items {
        if remaining.signum() <= 0 {
            break;
        }
        let take = if cmp_qty(w, remaining)? == Ordering::Greater {
            remaining
        } else {
            w
        };
        weighted_tail = checked_add(weighted_tail, checked_mul(take, loss)?)?;
        remaining = checked_sub(remaining, take)?;
    }

    checked_div(weighted_tail, alpha)
}

/// Compute both rules from the approval information set alone. Callers can retain the
/// returned digest before collecting an outcome. This library does not record it in a ledger.
pub fn freeze_approval<T>(case: &ApprovalCase<T>) -> Result<FrozenApproval, Refusal> {
    validate_token("case_id", &case.case_id)?;
    validate_token("project_name", &case.project_name)?;
    if case.approval_date != utc_date(case.approval_as_of_ns) {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            format!(
                "case {}: approval_date must match the UTC day of approval_as_of_ns",
                case.case_id
            ),
        ));
    }
    ValidTime::new(case.physical_horizon.start_ns, case.physical_horizon.end_ns).map_err(|e| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("case {}: invalid physical_horizon: {e}", case.case_id),
        )
    })?;
    if case.h7a_hurdle_irr.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("case {}: h7a_hurdle_irr must be >= 0", case.case_id),
        ));
    }
    if case.baseline_capex_usd_m.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("case {}: baseline_capex_usd_m must be > 0", case.case_id),
        ));
    }
    if case.shares_millions.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("case {}: shares_millions must be > 0", case.case_id),
        ));
    }
    if case.max_cvar_loss_per_share_usd.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "case {}: max_cvar_loss_per_share_usd must be >= 0",
                case.case_id
            ),
        ));
    }
    if case.approval_evidence.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!(
                "case {}: at least one timestamped approval_evidence record is required",
                case.case_id
            ),
        ));
    }
    if case.scenarios.len() < 2 {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!(
                "case {}: at least 2 shared scenarios are required for H7d",
                case.case_id
            ),
        ));
    }

    // 1. Strict INV-016 / G-TIME-2 as_of_decision(t_approval) check on EVERY approval evidence row.
    let query = AsOfQuery {
        as_of_decision_ns: case.approval_as_of_ns,
        anchor: case.approval_anchor.clone(),
        for_physical_period: case.physical_horizon,
        purpose: QueryPurpose::OperationalDecision,
    };
    let mut evidence = BTreeMap::new();
    for ev in &case.approval_evidence {
        validate_token("approval_evidence.evidence_id", &ev.evidence_id)?;
        validate_token("approval_evidence.label", &ev.label)?;
        if evidence.insert(ev.evidence_id.as_str(), ev).is_some() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "case {}: duplicate approval_evidence id {:?}",
                    case.case_id, ev.evidence_id
                ),
            ));
        }
        match query.check_admissible(&ev.stamp) {
            Ok(true) => {}
            Ok(false) => {
                return Err(Refusal::new(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "case {}: evidence '{}' is outside physical horizon or uncommitted at approval anchor",
                        case.case_id, ev.evidence_id
                    ),
                ));
            }
            Err(TimeError::AssayLagLookahead {
                availability_time,
                as_of_decision_ns,
            }) => {
                return Err(Refusal::new(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "case {}: hindsight leak in evidence '{}' (availability_time={availability_time} > approval_as_of_ns={as_of_decision_ns})",
                        case.case_id, ev.evidence_id
                    ),
                ));
            }
            Err(other) => {
                return Err(Refusal::new(
                    RefusalCode::AssayLookahead108,
                    format!(
                        "case {}: approval evidence '{}' failed as_of check: {other}",
                        case.case_id, ev.evidence_id
                    ),
                ));
            }
        }
    }

    // Exact sums and the digest both follow scenario identity order.
    let mut scenarios = BTreeMap::new();
    for s in &case.scenarios {
        validate_token("scenario_id", &s.scenario_id)?;
        if scenarios.insert(s.scenario_id.as_str(), s).is_some() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "case {}: duplicate scenario_id {:?}",
                    case.case_id, s.scenario_id
                ),
            ));
        }
        if s.weight.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "case {}: scenario '{}' weight must be > 0",
                    case.case_id, s.scenario_id
                ),
            ));
        }
    }
    let weight_sum = scenarios
        .values()
        .try_fold(ZERO, |sum, scenario| checked_add(sum, scenario.weight))?;
    if weight_sum != ONE {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "case {}: scenario weights must sum to 1, got {}/{}",
                case.case_id,
                weight_sum.num(),
                weight_sum.den()
            ),
        ));
    }

    // 4. Freeze H7a decision (static NPV > 0 and IRR >= hurdle, ignoring overrun bias & recourse).
    let h7a_decision = if case.h7a_static_npv_usd_m.signum() > 0
        && cmp_qty(case.h7a_static_irr, case.h7a_hurdle_irr)? != Ordering::Less
    {
        ApprovalAction::ApproveFull
    } else {
        ApprovalAction::DeferOrPass
    };

    // 5. Freeze H7d decision (per-share expected ΔNAV subject to liquidity floor and CVaR limit).
    let action_metrics = [
        evaluate_action(case, &scenarios, ApprovalAction::ApproveFull)?,
        evaluate_action(case, &scenarios, ApprovalAction::StageWithRecourse)?,
        evaluate_action(case, &scenarios, ApprovalAction::DeferOrPass)?,
    ];

    let static_expected = action_metrics[0].expected_delta_nav_per_share_usd;
    let recourse_option_value = checked_sub(
        action_metrics[1].expected_delta_nav_per_share_usd,
        static_expected,
    )?;

    // Choose highest-E[ΔNAV/share] feasible action; exact ties prefer StageWithRecourse > DeferOrPass > ApproveFull.
    let preference_rank = |a: ApprovalAction| -> u8 {
        match a {
            ApprovalAction::StageWithRecourse => 3,
            ApprovalAction::DeferOrPass => 2,
            ApprovalAction::ApproveFull => 1,
        }
    };
    let mut best_h7d: Option<&ActionApprovalMetrics> = None;
    for m in &action_metrics {
        if !m.feasible {
            continue;
        }
        best_h7d = match best_h7d {
            None => Some(m),
            Some(cur) => match cmp_qty(
                m.expected_delta_nav_per_share_usd,
                cur.expected_delta_nav_per_share_usd,
            )? {
                Ordering::Greater => Some(m),
                Ordering::Equal if preference_rank(m.action) > preference_rank(cur.action) => {
                    Some(m)
                }
                _ => Some(cur),
            },
        };
    }
    let h7d_decision = best_h7d
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!(
                    "case {}: no action satisfies both liquidity floor and CVaR limit under H7d",
                    case.case_id
                ),
            )
        })?
        .action;

    let mut band_min = case.scenarios[0].full_commit_delta_nav_usd_m;
    let mut band_max = band_min;
    for s in scenarios.values() {
        if cmp_qty(s.full_commit_delta_nav_usd_m, band_min)? == Ordering::Less {
            band_min = s.full_commit_delta_nav_usd_m;
        }
        if cmp_qty(s.full_commit_delta_nav_usd_m, band_max)? == Ordering::Greater {
            band_max = s.full_commit_delta_nav_usd_m;
        }
    }
    Ok(FrozenApproval {
        case_id: case.case_id.clone(),
        project_name: case.project_name.clone(),
        approval_date: case.approval_date,
        approval_as_of_ns: case.approval_as_of_ns,
        is_synthetic_scenario_grid: case.is_synthetic_scenario_grid,
        admitted_evidence_ids: evidence.keys().map(|id| (*id).to_owned()).collect(),
        digest: approval_digest(case, &evidence, &scenarios, [h7a_decision, h7d_decision]),
        h7a_decision,
        h7d_decision,
        action_metrics,
        recourse_option_value,
        baseline_capex_usd_m: case.baseline_capex_usd_m,
        shares_millions: case.shares_millions,
        full_commit_nav_band: [band_min, band_max],
    })
}

impl FrozenApproval {
    /// Score these fixed selections using supplied payoffs. The caller must associate the
    /// outcome with this approval. Payoffs for unchosen actions are counterfactual.
    pub fn score(&self, outcome: &RealizedApprovalOutcome) -> Result<HindcastCaseResult, Refusal> {
        let case = &self.case_id;
        let stamp = &outcome.realized_stamp;
        let resolved_at = stamp.physical_valid_time().end_ns;
        // INV-008 concerns physical resolution. Delayed publication cannot make a prior
        // realization prospective. The outcome must also be available by its stated clock.
        if resolved_at <= self.approval_as_of_ns
            || stamp.availability_time() < resolved_at
            || outcome.realized_date != utc_date(resolved_at)
        {
            return Err(Refusal::new(
                RefusalCode::AssayLookahead108,
                format!(
                    "case {case}: under INV-008 the outcome must resolve strictly after the approval cut, match realized_date in UTC, and become available at or after resolution"
                ),
            ));
        }
        if outcome.realized_capex_usd_m.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("case {case}: realized_capex_usd_m must be > 0"),
            ));
        }
        if outcome.outcome_evidence_ids.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("case {case}: outcome_evidence_ids must be non-empty"),
            ));
        }
        let mut seen_outcome_ids = BTreeSet::new();
        for eid in &outcome.outcome_evidence_ids {
            validate_token("outcome_evidence_id", eid)?;
            if !seen_outcome_ids.insert(eid.as_str()) {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("case {case}: duplicate outcome_evidence_id {eid:?}"),
                ));
            }
        }
        let outcome_evidence_ids: Vec<_> =
            seen_outcome_ids.into_iter().map(str::to_owned).collect();
        self.score_payoffs(outcome, outcome_evidence_ids)
    }

    fn score_payoffs(
        &self,
        outcome: &RealizedApprovalOutcome,
        outcome_evidence_ids: Vec<String>,
    ) -> Result<HindcastCaseResult, Refusal> {
        let realized_for = |action: ApprovalAction| -> Result<ExactQty, Refusal> {
            let nav = match action {
                ApprovalAction::ApproveFull => outcome.realized_full_commit_delta_nav_usd_m,
                ApprovalAction::StageWithRecourse => {
                    outcome.realized_staged_recourse_delta_nav_usd_m
                }
                ApprovalAction::DeferOrPass => outcome.realized_defer_delta_nav_usd_m,
            };
            checked_div(nav, self.shares_millions)
        };
        let r_full = realized_for(ApprovalAction::ApproveFull)?;
        let r_stage = realized_for(ApprovalAction::StageWithRecourse)?;
        let r_defer = realized_for(ApprovalAction::DeferOrPass)?;
        let mut best_hindsight = r_full;
        if cmp_qty(r_stage, best_hindsight)? == Ordering::Greater {
            best_hindsight = r_stage;
        }
        if cmp_qty(r_defer, best_hindsight)? == Ordering::Greater {
            best_hindsight = r_defer;
        }

        let h7a_realized = realized_for(self.h7a_decision)?;
        let h7d_realized = realized_for(self.h7d_decision)?;
        let h7a_regret = checked_sub(best_hindsight, h7a_realized)?;
        let h7d_regret = checked_sub(best_hindsight, h7d_realized)?;
        let h7d_advantage = checked_sub(h7d_realized, h7a_realized)?;
        let realized_capex_multiple =
            checked_div(outcome.realized_capex_usd_m, self.baseline_capex_usd_m)?;

        let [band_min, band_max] = self.full_commit_nav_band;
        let realized_within_approval_band =
            cmp_qty(outcome.realized_full_commit_delta_nav_usd_m, band_min)? != Ordering::Less
                && cmp_qty(outcome.realized_full_commit_delta_nav_usd_m, band_max)?
                    != Ordering::Greater;

        let outcome_digest = outcome_digest(outcome, &outcome_evidence_ids);
        Ok(HindcastCaseResult {
            case_id: self.case_id.clone(),
            project_name: self.project_name.clone(),
            approval_date: self.approval_date,
            is_synthetic_scenario_grid: self.is_synthetic_scenario_grid,
            admitted_evidence_ids: self.admitted_evidence_ids.clone(),
            outcome_evidence_ids,
            approval_digest: self.digest,
            outcome_digest,
            h7a_decision: self.h7a_decision,
            h7d_decision: self.h7d_decision,
            decisions_diverged: self.h7a_decision != self.h7d_decision,
            static_expected_delta_nav_per_share_usd: self.action_metrics[0]
                .expected_delta_nav_per_share_usd,
            recourse_option_value_per_share_usd: self.recourse_option_value,
            action_metrics: self.action_metrics.clone(),
            realized_capex_multiple,
            schedule_slip_months: outcome.schedule_slip_months,
            h7a_realized_delta_nav_per_share_usd: h7a_realized,
            h7d_realized_delta_nav_per_share_usd: h7d_realized,
            best_hindsight_delta_nav_per_share_usd: best_hindsight,
            h7a_regret_per_share_usd: h7a_regret,
            h7d_regret_per_share_usd: h7d_regret,
            h7d_advantage_per_share_usd: h7d_advantage,
            realized_within_approval_band,
        })
    }
}

fn evaluate_action<T>(
    case: &ApprovalCase<T>,
    scenarios: &BTreeMap<&str, &HindcastScenario>,
    action: ApprovalAction,
) -> Result<ActionApprovalMetrics, Refusal> {
    let mut expected_per_share = ZERO;
    let mut weighted_losses = Vec::with_capacity(case.scenarios.len());
    let mut min_slack: Option<ExactQty> = None;

    for s in scenarios.values() {
        let (nav_usd_m, slack_usd_m) = match action {
            ApprovalAction::ApproveFull => (
                s.full_commit_delta_nav_usd_m,
                s.full_commit_liquidity_slack_usd_m,
            ),
            ApprovalAction::StageWithRecourse => (
                s.staged_recourse_delta_nav_usd_m,
                s.staged_recourse_liquidity_slack_usd_m,
            ),
            ApprovalAction::DeferOrPass => (s.defer_delta_nav_usd_m, s.defer_liquidity_slack_usd_m),
        };
        let per_share = checked_div(nav_usd_m, case.shares_millions)?;
        expected_per_share = checked_add(expected_per_share, checked_mul(s.weight, per_share)?)?;
        let signed_loss = per_share.checked_neg().map_err(units_err)?;
        weighted_losses.push((s.weight, signed_loss));
        min_slack = Some(match min_slack {
            None => slack_usd_m,
            Some(cur) if cmp_qty(slack_usd_m, cur)? == Ordering::Less => slack_usd_m,
            Some(cur) => cur,
        });
    }

    let cvar_loss = exact_weighted_cvar(&weighted_losses, case.cvar_tail_mass)?;
    let min_liquidity_slack_usd_m = min_slack.unwrap_or(ZERO);
    let liquidity_feasible = min_liquidity_slack_usd_m.signum() >= 0;
    let cvar_feasible = cmp_qty(cvar_loss, case.max_cvar_loss_per_share_usd)? != Ordering::Greater;
    Ok(ActionApprovalMetrics {
        action,
        expected_delta_nav_per_share_usd: expected_per_share,
        cvar_loss_per_share_usd: cvar_loss,
        min_liquidity_slack_usd_m,
        liquidity_feasible,
        cvar_feasible,
        feasible: liquidity_feasible && cvar_feasible,
    })
}

#[allow(clippy::too_many_arguments)]
fn compute_scorecard_digest(
    results: &[HindcastCaseResult],
    synthetic_count: usize,
    divergent_count: usize,
    band_covered_count: usize,
    h7a_total_realized: ExactQty,
    h7d_total_realized: ExactQty,
    h7a_total_regret: ExactQty,
    h7d_total_regret: ExactQty,
    h7d_net_advantage: ExactQty,
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"BTH1", 2);
    enc.put_u64(results.len() as u64);
    enc.put_u64(synthetic_count as u64);
    enc.put_u64(divergent_count as u64);
    enc.put_u64(band_covered_count as u64);
    put_qty(&mut enc, h7a_total_realized);
    put_qty(&mut enc, h7d_total_realized);
    put_qty(&mut enc, h7a_total_regret);
    put_qty(&mut enc, h7d_total_regret);
    put_qty(&mut enc, h7d_net_advantage);
    for r in results {
        enc.put_bytes(r.approval_digest.as_bytes());
        enc.put_bytes(r.outcome_digest.as_bytes());
        enc.put_str(&r.case_id);
        enc.put_str(&r.project_name);
        enc.put_i64(i64::from(r.approval_date.year));
        enc.put_u8(r.approval_date.month);
        enc.put_u8(r.approval_date.day);
        enc.put_u8(u8::from(r.is_synthetic_scenario_grid));
        enc.put_u64(r.admitted_evidence_ids.len() as u64);
        for eid in &r.admitted_evidence_ids {
            enc.put_str(eid);
        }
        enc.put_u64(r.outcome_evidence_ids.len() as u64);
        for eid in &r.outcome_evidence_ids {
            enc.put_str(eid);
        }
        enc.put_u8(r.h7a_decision.tag());
        enc.put_u8(r.h7d_decision.tag());
        put_qty(&mut enc, r.static_expected_delta_nav_per_share_usd);
        put_qty(&mut enc, r.recourse_option_value_per_share_usd);
        for m in &r.action_metrics {
            enc.put_u8(m.action.tag());
            put_qty(&mut enc, m.expected_delta_nav_per_share_usd);
            put_qty(&mut enc, m.cvar_loss_per_share_usd);
            put_qty(&mut enc, m.min_liquidity_slack_usd_m);
            enc.put_u8(u8::from(m.liquidity_feasible));
            enc.put_u8(u8::from(m.cvar_feasible));
            enc.put_u8(u8::from(m.feasible));
        }
        put_qty(&mut enc, r.realized_capex_multiple);
        enc.put_i64(i64::from(r.schedule_slip_months));
        put_qty(&mut enc, r.h7a_realized_delta_nav_per_share_usd);
        put_qty(&mut enc, r.h7d_realized_delta_nav_per_share_usd);
        put_qty(&mut enc, r.best_hindsight_delta_nav_per_share_usd);
        put_qty(&mut enc, r.h7a_regret_per_share_usd);
        put_qty(&mut enc, r.h7d_regret_per_share_usd);
        put_qty(&mut enc, r.h7d_advantage_per_share_usd);
        enc.put_u8(u8::from(r.realized_within_approval_band));
    }
    sha256(&enc.finish())
}

fn approval_digest<T>(
    case: &ApprovalCase<T>,
    evidence: &BTreeMap<&str, &ApprovalEvidence>,
    scenarios: &BTreeMap<&str, &HindcastScenario>,
    decisions: [ApprovalAction; 2],
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"BTHA", 1);
    enc.put_str(&case.case_id);
    enc.put_str(&case.project_name);
    enc.put_i64(case.approval_as_of_ns);
    put_date(&mut enc, case.approval_date);
    put_interval(&mut enc, case.physical_horizon);
    enc.put_u8(u8::from(case.is_synthetic_scenario_grid));
    for qty in [
        case.baseline_capex_usd_m,
        case.shares_millions,
        case.h7a_static_npv_usd_m,
        case.h7a_static_irr,
        case.h7a_hurdle_irr,
        case.cvar_tail_mass,
        case.max_cvar_loss_per_share_usd,
    ] {
        put_qty(&mut enc, qty);
    }
    enc.put_u64(evidence.len() as u64);
    for row in evidence.values() {
        enc.put_str(&row.evidence_id);
        enc.put_str(&row.label);
        enc.put_bytes(&row.evidence_class.tag());
        put_stamp(&mut enc, &row.stamp);
        // Only marks consulted by the admission check enter this commitment.
        if let Some(mark) = case.approval_anchor.mark(row.stamp.record_time().cell()) {
            enc.put_u8(1);
            enc.put_u64(mark);
        } else {
            enc.put_u8(0);
        }
    }
    enc.put_u64(scenarios.len() as u64);
    for row in scenarios.values() {
        enc.put_str(&row.scenario_id);
        for qty in [
            row.weight,
            row.full_commit_delta_nav_usd_m,
            row.staged_recourse_delta_nav_usd_m,
            row.defer_delta_nav_usd_m,
            row.full_commit_liquidity_slack_usd_m,
            row.staged_recourse_liquidity_slack_usd_m,
            row.defer_liquidity_slack_usd_m,
        ] {
            put_qty(&mut enc, qty);
        }
    }
    for decision in decisions {
        enc.put_u8(decision.tag());
    }
    sha256(&enc.finish())
}

fn outcome_digest(outcome: &RealizedApprovalOutcome, evidence_ids: &[String]) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"BTHO", 1);
    put_date(&mut enc, outcome.realized_date);
    put_stamp(&mut enc, &outcome.realized_stamp);
    enc.put_i64(i64::from(outcome.schedule_slip_months));
    for qty in [
        outcome.realized_capex_usd_m,
        outcome.realized_full_commit_delta_nav_usd_m,
        outcome.realized_staged_recourse_delta_nav_usd_m,
        outcome.realized_defer_delta_nav_usd_m,
    ] {
        put_qty(&mut enc, qty);
    }
    enc.put_u64(evidence_ids.len() as u64);
    for id in evidence_ids {
        enc.put_str(id);
    }
    sha256(&enc.finish())
}

fn utc_date(ns: i64) -> CivilDate {
    CivilDate::from_epoch_days(ns.div_euclid(NS_PER_DAY))
}

fn put_date(enc: &mut Encoder, date: CivilDate) {
    enc.put_i64(i64::from(date.year));
    enc.put_u8(date.month);
    enc.put_u8(date.day);
}

fn put_interval(enc: &mut Encoder, interval: ValidTime) {
    enc.put_i64(interval.start_ns);
    enc.put_i64(interval.end_ns);
}

fn put_stamp(enc: &mut Encoder, stamp: &QuadTemporalStamp) {
    put_interval(enc, stamp.physical_valid_time());
    enc.put_i64(stamp.collection_time());
    enc.put_i64(stamp.availability_time());
    let record = stamp.record_time();
    enc.put_str(record.cell());
    enc.put_u64(record.seq());
    enc.put_i64(record.wall_ns());
    match record.evidence() {
        TimeEvidence::HostSynced => enc.put_u8(0),
        TimeEvidence::NtpBounded { max_skew_ms } => {
            enc.put_u8(1);
            enc.put_u64(u64::from(max_skew_ms));
        }
        TimeEvidence::OfflineUncertain => enc.put_u8(2),
    }
}

fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_i128(q.num());
    enc.put_i128(q.den());
}

fn validate_token(field: &str, value: &str) -> Result<(), Refusal> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.bytes().any(|b| b.is_ascii_control())
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "{field} must be non-empty, trimmed, and free of control characters, got {value:?}"
            ),
        ));
    }
    Ok(())
}

fn units_err(err: fmine_units::UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::NumericalCertificateFailed207,
        format!("exact hindcast arithmetic error: {err}"),
    )
}

fn checked_add(a: ExactQty, b: ExactQty) -> Result<ExactQty, Refusal> {
    a.checked_add(b).map_err(units_err)
}

fn checked_sub(a: ExactQty, b: ExactQty) -> Result<ExactQty, Refusal> {
    a.checked_sub(b).map_err(units_err)
}

fn checked_mul(a: ExactQty, b: ExactQty) -> Result<ExactQty, Refusal> {
    a.checked_mul(b).map_err(units_err)
}

fn checked_div(a: ExactQty, b: ExactQty) -> Result<ExactQty, Refusal> {
    a.checked_div(b).map_err(units_err)
}

fn cmp_qty(a: ExactQty, b: ExactQty) -> Result<Ordering, Refusal> {
    a.checked_cmp(b).map_err(units_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_time::{RecordTime, TimeEvidence};

    const DAY_NS: i64 = 86_400 * 1_000_000_000;

    fn q(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).expect("valid rational")
    }

    fn date_ns(y: i32, m: u8, d: u8) -> i64 {
        CivilDate::new(y, m, d).expect("valid date").to_epoch_days() * DAY_NS
    }

    fn mk_stamp(
        phys_start: i64,
        phys_end: i64,
        avail: i64,
        cell: &str,
        seq: u64,
    ) -> QuadTemporalStamp {
        let vt = ValidTime::new(phys_start, phys_end).expect("vt");
        let rec = RecordTime::new(cell, seq, avail, TimeEvidence::HostSynced);
        QuadTemporalStamp::new(vt, phys_start, avail, rec).expect("stamp")
    }

    /// Five-case benchmark suite combining plan §23.1 cited `FACT` overrun/impairment anchors
    /// (`E631`, `E632`, `E633`, `E642`, `GB-21`) with explicitly tagged synthetic/assumed
    /// approval-time scenario grids (`is_synthetic_scenario_grid = true`).
    fn benchmark_five_cases() -> Vec<ApprovalCase> {
        let cell = "helm-capex-hindcast";
        let horizon = ValidTime::new(date_ns(2005, 1, 1), date_ns(2030, 1, 1)).unwrap();

        // Case 1: Tanami Expansion 2 (FACT E631/E632: Feb 2022 estimate $850-950M, midpoint $900M;
        // Feb 2026 estimate $1.7-1.8B, midpoint $1,750M -> ~1.94x overrun, +30 mo slip).
        // Approval-time scenario grid is ASSUMED/SYNTHETIC.
        let t1_app = date_ns(2022, 2, 24);
        let c1 = ApprovalCase {
            case_id: "cap-01-tanami2-anchor".into(),
            project_name: "Tanami Expansion 2 (E631/E632 anchor, synthetic scenario grid)".into(),
            approval_date: CivilDate::new(2022, 2, 24).unwrap(),
            approval_as_of_ns: t1_app,
            approval_anchor: Anchor::new([(cell.to_string(), 10)]).expect("anchor"),
            physical_horizon: horizon,
            is_synthetic_scenario_grid: true,
            approval_evidence: vec![ApprovalEvidence {
                evidence_id: "E631-feb2022-est".into(),
                label: "Feb 2022 capex estimate midpoint $900M ($850-950M)".into(),
                evidence_class: EvidenceClass::ISSUER,
                stamp: mk_stamp(date_ns(2022, 1, 1), date_ns(2022, 2, 24), t1_app, cell, 10),
            }],
            baseline_capex_usd_m: q(900, 1),
            shares_millions: q(1000, 1),
            h7a_static_npv_usd_m: q(450, 1),
            h7a_static_irr: q(20, 100),
            h7a_hurdle_irr: q(15, 100),
            scenarios: vec![
                HindcastScenario {
                    scenario_id: "s1-deep-shaft-overrun".into(),
                    weight: q(3, 10),
                    full_commit_delta_nav_usd_m: q(-600, 1),
                    staged_recourse_delta_nav_usd_m: q(-120, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(100, 1),
                    staged_recourse_liquidity_slack_usd_m: q(300, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s2-moderate-overrun".into(),
                    weight: q(4, 10),
                    full_commit_delta_nav_usd_m: q(150, 1),
                    staged_recourse_delta_nav_usd_m: q(180, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(250, 1),
                    staged_recourse_liquidity_slack_usd_m: q(350, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s3-on-budget".into(),
                    weight: q(3, 10),
                    full_commit_delta_nav_usd_m: q(500, 1),
                    staged_recourse_delta_nav_usd_m: q(420, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(400, 1),
                    staged_recourse_liquidity_slack_usd_m: q(420, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
            ],
            cvar_tail_mass: q(1, 5),
            max_cvar_loss_per_share_usd: q(30, 100), // $0.30/share tail-loss limit
            realized_outcome: RealizedApprovalOutcome {
                realized_date: CivilDate::new(2026, 2, 19).unwrap(),
                realized_stamp: mk_stamp(
                    date_ns(2022, 2, 24),
                    date_ns(2026, 2, 19),
                    date_ns(2026, 2, 19),
                    cell,
                    20,
                ),
                realized_capex_usd_m: q(1750, 1),
                schedule_slip_months: 30,
                realized_full_commit_delta_nav_usd_m: q(-500, 1),
                realized_staged_recourse_delta_nav_usd_m: q(-100, 1),
                realized_defer_delta_nav_usd_m: ZERO,
                outcome_evidence_ids: vec!["E631".into(), "E632".into()],
            },
        };

        // Case 2: Pascua-Lama high-altitude cross-border permit/capex tail (FACT E633: $6.1B 2013 impairment).
        let t2_app = date_ns(2009, 5, 7);
        let c2 = ApprovalCase {
            case_id: "cap-02-pascua-lama-anchor".into(),
            project_name: "Pascua-Lama (E633 impairment anchor, synthetic scenario grid)".into(),
            approval_date: CivilDate::new(2009, 5, 7).unwrap(),
            approval_as_of_ns: t2_app,
            approval_anchor: Anchor::new([(cell.to_string(), 10)]).expect("anchor"),
            physical_horizon: horizon,
            is_synthetic_scenario_grid: true,
            approval_evidence: vec![ApprovalEvidence {
                evidence_id: "E633-approval-baseline".into(),
                label: "Synthetic approval baseline prior to 2013 $6.1B impairment".into(),
                evidence_class: EvidenceClass::Assumed,
                stamp: mk_stamp(date_ns(2009, 1, 1), date_ns(2009, 5, 7), t2_app, cell, 10),
            }],
            baseline_capex_usd_m: q(3000, 1),
            shares_millions: q(1000, 1),
            h7a_static_npv_usd_m: q(2200, 1),
            h7a_static_irr: q(22, 100),
            h7a_hurdle_irr: q(15, 100),
            scenarios: vec![
                HindcastScenario {
                    scenario_id: "s1-permit-halt-and-overrun".into(),
                    weight: q(25, 100),
                    full_commit_delta_nav_usd_m: q(-5200, 1),
                    staged_recourse_delta_nav_usd_m: q(-400, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(-800, 1), // breaches cash floor
                    staged_recourse_liquidity_slack_usd_m: q(200, 1),
                    defer_liquidity_slack_usd_m: q(600, 1),
                },
                HindcastScenario {
                    scenario_id: "s2-base-build".into(),
                    weight: q(50, 100),
                    full_commit_delta_nav_usd_m: q(1800, 1),
                    staged_recourse_delta_nav_usd_m: q(1400, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(400, 1),
                    staged_recourse_liquidity_slack_usd_m: q(500, 1),
                    defer_liquidity_slack_usd_m: q(600, 1),
                },
                HindcastScenario {
                    scenario_id: "s3-bull-gold".into(),
                    weight: q(25, 100),
                    full_commit_delta_nav_usd_m: q(3600, 1),
                    staged_recourse_delta_nav_usd_m: q(3000, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(800, 1),
                    staged_recourse_liquidity_slack_usd_m: q(800, 1),
                    defer_liquidity_slack_usd_m: q(600, 1),
                },
            ],
            cvar_tail_mass: q(1, 5),
            max_cvar_loss_per_share_usd: q(100, 100), // $1.00/share tail limit
            realized_outcome: RealizedApprovalOutcome {
                realized_date: CivilDate::new(2013, 12, 31).unwrap(),
                realized_stamp: mk_stamp(
                    date_ns(2013, 1, 1),
                    date_ns(2013, 12, 31),
                    date_ns(2014, 2, 13),
                    cell,
                    25,
                ),
                realized_capex_usd_m: q(8500, 1),
                schedule_slip_months: 60,
                realized_full_commit_delta_nav_usd_m: q(-6100, 1),
                realized_staged_recourse_delta_nav_usd_m: q(-400, 1),
                realized_defer_delta_nav_usd_m: ZERO,
                outcome_evidence_ids: vec!["E633".into()],
            },
        };

        // Case 3: Greenfield copper build-vs-buy parity (+40% overrun benchmark, FACT E642).
        let t3_app = date_ns(2021, 6, 1);
        let c3 = ApprovalCase {
            case_id: "cap-03-greenfield-copper-e642".into(),
            project_name: "Synthetic Greenfield Copper (E642 +40% overrun benchmark)".into(),
            approval_date: CivilDate::new(2021, 6, 1).unwrap(),
            approval_as_of_ns: t3_app,
            approval_anchor: Anchor::new([(cell.to_string(), 10)]).expect("anchor"),
            physical_horizon: horizon,
            is_synthetic_scenario_grid: true,
            approval_evidence: vec![ApprovalEvidence {
                evidence_id: "E642-refclass-overrun".into(),
                label: "Greenfield copper median $18,200/tpa + 40% overrun benchmark".into(),
                evidence_class: EvidenceClass::Assumed,
                stamp: mk_stamp(date_ns(2021, 1, 1), date_ns(2021, 6, 1), t3_app, cell, 10),
            }],
            baseline_capex_usd_m: q(2000, 1),
            shares_millions: q(1000, 1),
            h7a_static_npv_usd_m: q(300, 1),
            h7a_static_irr: q(16, 100),
            h7a_hurdle_irr: q(15, 100),
            scenarios: vec![
                HindcastScenario {
                    scenario_id: "s1-40pct-overrun-low-cu".into(),
                    weight: q(4, 10),
                    full_commit_delta_nav_usd_m: q(-700, 1),
                    staged_recourse_delta_nav_usd_m: q(-150, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(50, 1),
                    staged_recourse_liquidity_slack_usd_m: q(250, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s2-base-cu".into(),
                    weight: q(4, 10),
                    full_commit_delta_nav_usd_m: q(200, 1),
                    staged_recourse_delta_nav_usd_m: q(100, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(200, 1),
                    staged_recourse_liquidity_slack_usd_m: q(300, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s3-high-cu".into(),
                    weight: q(2, 10),
                    full_commit_delta_nav_usd_m: q(700, 1),
                    staged_recourse_delta_nav_usd_m: q(450, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(400, 1),
                    staged_recourse_liquidity_slack_usd_m: q(450, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
            ],
            cvar_tail_mass: q(1, 5),
            max_cvar_loss_per_share_usd: q(25, 100),
            realized_outcome: RealizedApprovalOutcome {
                realized_date: CivilDate::new(2025, 6, 1).unwrap(),
                realized_stamp: mk_stamp(
                    date_ns(2024, 1, 1),
                    date_ns(2025, 6, 1),
                    date_ns(2025, 7, 1),
                    cell,
                    30,
                ),
                realized_capex_usd_m: q(2800, 1), // 1.40x overrun
                schedule_slip_months: 18,
                realized_full_commit_delta_nav_usd_m: q(-650, 1),
                realized_staged_recourse_delta_nav_usd_m: q(-140, 1),
                realized_defer_delta_nav_usd_m: ZERO,
                outcome_evidence_ids: vec!["E642".into()],
            },
        };

        // Case 4: Brownfield mill debottleneck — both H7a and H7d agree on ApproveFull (controlled risk).
        let t4_app = date_ns(2020, 9, 1);
        let c4 = ApprovalCase {
            case_id: "cap-04-brownfield-debottleneck".into(),
            project_name: "Synthetic Brownfield Mill Debottleneck".into(),
            approval_date: CivilDate::new(2020, 9, 1).unwrap(),
            approval_as_of_ns: t4_app,
            approval_anchor: Anchor::new([(cell.to_string(), 10)]).expect("anchor"),
            physical_horizon: horizon,
            is_synthetic_scenario_grid: true,
            approval_evidence: vec![ApprovalEvidence {
                evidence_id: "SYN-brownfield-fs".into(),
                label: "Synthetic brownfield feasibility study".into(),
                evidence_class: EvidenceClass::Assumed,
                stamp: mk_stamp(date_ns(2020, 1, 1), date_ns(2020, 9, 1), t4_app, cell, 10),
            }],
            baseline_capex_usd_m: q(250, 1),
            shares_millions: q(1000, 1),
            h7a_static_npv_usd_m: q(320, 1),
            h7a_static_irr: q(28, 100),
            h7a_hurdle_irr: q(15, 100),
            scenarios: vec![
                HindcastScenario {
                    scenario_id: "s1-mild-overrun".into(),
                    weight: q(2, 10),
                    full_commit_delta_nav_usd_m: q(40, 1),
                    staged_recourse_delta_nav_usd_m: q(30, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(450, 1),
                    staged_recourse_liquidity_slack_usd_m: q(460, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s2-base".into(),
                    weight: q(5, 10),
                    full_commit_delta_nav_usd_m: q(300, 1),
                    staged_recourse_delta_nav_usd_m: q(240, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(500, 1),
                    staged_recourse_liquidity_slack_usd_m: q(500, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s3-upside".into(),
                    weight: q(3, 10),
                    full_commit_delta_nav_usd_m: q(480, 1),
                    staged_recourse_delta_nav_usd_m: q(380, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(600, 1),
                    staged_recourse_liquidity_slack_usd_m: q(580, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
            ],
            cvar_tail_mass: q(1, 5),
            max_cvar_loss_per_share_usd: q(30, 100),
            realized_outcome: RealizedApprovalOutcome {
                realized_date: CivilDate::new(2022, 12, 1).unwrap(),
                realized_stamp: mk_stamp(
                    date_ns(2022, 1, 1),
                    date_ns(2022, 12, 1),
                    date_ns(2023, 1, 15),
                    cell,
                    35,
                ),
                realized_capex_usd_m: q(280, 1), // 1.12x
                schedule_slip_months: 3,
                realized_full_commit_delta_nav_usd_m: q(290, 1),
                realized_staged_recourse_delta_nav_usd_m: q(230, 1),
                realized_defer_delta_nav_usd_m: ZERO,
                outcome_evidence_ids: vec!["SYN-brownfield-closeout".into()],
            },
        };

        // Case 5: Sustaining TSF buttress (plan §23.2: risk reduction, negative static NPV under H7a,
        // chosen by H7d because deferring breaches the CVaR tail-loss constraint!).
        let t5_app = date_ns(2021, 3, 1);
        let c5 = ApprovalCase {
            case_id: "cap-05-tsf-sustaining-buttress".into(),
            project_name: "Synthetic Sustaining TSF Buttress (plan §23.2 risk reduction)".into(),
            approval_date: CivilDate::new(2021, 3, 1).unwrap(),
            approval_as_of_ns: t5_app,
            approval_anchor: Anchor::new([(cell.to_string(), 10)]).expect("anchor"),
            physical_horizon: horizon,
            is_synthetic_scenario_grid: true,
            approval_evidence: vec![ApprovalEvidence {
                evidence_id: "SYN-tsf-hazard".into(),
                label: "Synthetic TSF piezometer hazard assessment".into(),
                evidence_class: EvidenceClass::Assumed,
                stamp: mk_stamp(date_ns(2021, 1, 1), date_ns(2021, 3, 1), t5_app, cell, 10),
            }],
            baseline_capex_usd_m: q(120, 1),
            shares_millions: q(1000, 1),
            h7a_static_npv_usd_m: q(-120, 1), // pure sustaining spend looks NPV-negative to H7a!
            h7a_static_irr: ZERO,
            h7a_hurdle_irr: q(15, 100),
            scenarios: vec![
                HindcastScenario {
                    scenario_id: "s1-loss-of-containment".into(),
                    weight: q(1, 10),
                    full_commit_delta_nav_usd_m: q(-120, 1),
                    staged_recourse_delta_nav_usd_m: q(-200, 1),
                    defer_delta_nav_usd_m: q(-3000, 1), // catastrophic TSF event if deferred
                    full_commit_liquidity_slack_usd_m: q(380, 1),
                    staged_recourse_liquidity_slack_usd_m: q(350, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
                HindcastScenario {
                    scenario_id: "s2-no-incident".into(),
                    weight: q(9, 10),
                    full_commit_delta_nav_usd_m: q(-120, 1),
                    staged_recourse_delta_nav_usd_m: q(-140, 1),
                    defer_delta_nav_usd_m: ZERO,
                    full_commit_liquidity_slack_usd_m: q(380, 1),
                    staged_recourse_liquidity_slack_usd_m: q(360, 1),
                    defer_liquidity_slack_usd_m: q(500, 1),
                },
            ],
            cvar_tail_mass: q(1, 5), // worst 20%: defer has 10% at -3.00 and 10% at 0 -> CVaR = 1.50/sh > 0.30!
            max_cvar_loss_per_share_usd: q(30, 100),
            realized_outcome: RealizedApprovalOutcome {
                realized_date: CivilDate::new(2023, 6, 1).unwrap(),
                realized_stamp: mk_stamp(
                    date_ns(2023, 1, 1),
                    date_ns(2023, 6, 1),
                    date_ns(2023, 6, 15),
                    cell,
                    40,
                ),
                realized_capex_usd_m: q(132, 1), // 1.10x
                schedule_slip_months: 2,
                realized_full_commit_delta_nav_usd_m: q(-132, 1),
                realized_staged_recourse_delta_nav_usd_m: q(-220, 1),
                realized_defer_delta_nav_usd_m: q(-3000, 1),
                outcome_evidence_ids: vec!["SYN-tsf-extreme-rain".into()],
            },
        };

        vec![c1, c2, c3, c4, c5]
    }

    #[test]
    fn test_bitemporal_hindcast_five_approvals_h7a_vs_h7d_and_digest_stable() {
        let cases = benchmark_five_cases();
        let card = evaluate_gate_cap_04_hindcast(&cases).expect("5-case GATE-CAP-04 hindcast");
        assert_eq!(card.cases_count, 5);
        assert_eq!(card.synthetic_scenario_cases_count, 5);
        // Diverged on 4 of 5 cases (Tanami 2, Pascua-Lama, Greenfield Cu, TSF Buttress); agreed on Brownfield.
        assert_eq!(card.divergent_decisions_count, 4);

        let by_id = |id: &str| card.results.iter().find(|r| r.case_id == id).unwrap();
        // Case 1 (Tanami 2): H7a ApproveFull -> -$0.50/sh; H7d StageWithRecourse (E=+$0.162/sh, CVaR=$0.12/sh) -> -$0.10/sh.
        let r1 = by_id("cap-01-tanami2-anchor");
        assert_eq!(r1.h7a_decision, ApprovalAction::ApproveFull);
        assert_eq!(r1.h7d_decision, ApprovalAction::StageWithRecourse);
        assert_eq!(r1.realized_capex_multiple, q(1750, 900));
        assert_eq!(r1.h7d_advantage_per_share_usd, q(400, 1000));
        assert!(r1.realized_within_approval_band);
        // G-CAP-D: recourse option value = E[Stage](162/1000) - E[Full](30/1000) = 132/1000 exact!
        assert_eq!(r1.static_expected_delta_nav_per_share_usd, q(30, 1000));
        assert_eq!(r1.recourse_option_value_per_share_usd, q(132, 1000));

        // Case 2 (Pascua-Lama): H7a ApproveFull -> -$6.10/sh; H7d StageWithRecourse (full breaches liquidity & CVaR) -> -$0.40/sh.
        let r2 = by_id("cap-02-pascua-lama-anchor");
        assert_eq!(r2.h7a_decision, ApprovalAction::ApproveFull);
        assert_eq!(r2.h7d_decision, ApprovalAction::StageWithRecourse);
        assert!(!r2.action_metrics[0].liquidity_feasible);
        assert!(!r2.action_metrics[0].cvar_feasible);
        assert_eq!(r2.h7d_advantage_per_share_usd, q(5700, 1000));

        // Case 3 (Greenfield Cu): H7a ApproveFull -> -$0.65/sh; H7d StageWithRecourse (E=+$0.07/sh, CVaR=$0.15/sh) -> -$0.14/sh.
        let r3 = by_id("cap-03-greenfield-copper-e642");
        assert_eq!(r3.h7a_decision, ApprovalAction::ApproveFull);
        assert_eq!(r3.h7d_decision, ApprovalAction::StageWithRecourse);
        assert_eq!(r3.h7d_advantage_per_share_usd, q(510, 1000));

        // Case 4 (Brownfield): both pick ApproveFull, 0 divergence, 0 regret.
        let r4 = by_id("cap-04-brownfield-debottleneck");
        assert_eq!(r4.h7a_decision, ApprovalAction::ApproveFull);
        assert_eq!(r4.h7d_decision, ApprovalAction::ApproveFull);
        assert_eq!(r4.h7d_advantage_per_share_usd, ZERO);

        // Case 5 (Sustaining TSF): H7a DeferOrPass (static NPV < 0) -> -$3.00/sh; H7d ApproveFull -> -$0.132/sh.
        let r5 = by_id("cap-05-tsf-sustaining-buttress");
        assert_eq!(r5.h7a_decision, ApprovalAction::DeferOrPass);
        assert_eq!(r5.h7d_decision, ApprovalAction::ApproveFull);
        assert!(!r5.action_metrics[2].cvar_feasible); // deferring breaches CVaR tail limit!
        assert_eq!(r5.h7d_advantage_per_share_usd, q(2868, 1000));

        // Total H7d advantage = +0.40 + 5.70 + 0.51 + 0.00 + 2.868 = +$9.478/share exact!
        assert_eq!(card.h7d_net_advantage_per_share_usd, q(9478, 1000));
        assert!(
            card.h7d_total_regret_per_share_usd
                .checked_cmp(card.h7a_total_regret_per_share_usd)
                .unwrap()
                == Ordering::Less
        );

        // Reversed input order produces byte-identical BTH1 digest.
        let mut rev = cases;
        rev.reverse();
        let card_rev = evaluate_gate_cap_04_hindcast(&rev).expect("reversed order");
        assert_eq!(card_rev.digest, card.digest);
    }

    #[test]
    fn test_hindcast_hindsight_leak_refuses_fmine_ref_108() {
        let mut cases = benchmark_five_cases();
        // Inject a 2026 cost-overrun observation into a 2022 approval snapshot!
        let leaked_ts = date_ns(2026, 2, 19);
        cases[0].approval_evidence.push(ApprovalEvidence {
            evidence_id: "E631-leaked-2026-overrun".into(),
            label: "Post-approval 2026 overrun leaked into 2022 snapshot".into(),
            evidence_class: EvidenceClass::ISSUER,
            stamp: mk_stamp(
                date_ns(2022, 1, 1),
                date_ns(2026, 2, 19),
                leaked_ts,
                "helm-capex-hindcast",
                10,
            ),
        });
        let err = evaluate_hindcast_suite(&cases[..1]).expect_err("hindsight leak must refuse");
        assert_eq!(err.code, "FMINE-REF-108");
        assert!(err.what.contains("hindsight leak"));
    }

    #[test]
    fn test_hindcast_register_before_resolve_refuses_pre_approval_outcome() {
        let mut cases = benchmark_five_cases();
        // INV-008 violation: realized outcome timestamp equals approval timestamp.
        let app_ns = cases[0].approval_as_of_ns;
        cases[0].realized_outcome.realized_stamp = mk_stamp(
            date_ns(2022, 1, 1),
            date_ns(2022, 2, 24),
            app_ns,
            "helm-capex-hindcast",
            10,
        );
        let err = evaluate_hindcast_suite(&cases[..1]).expect_err("pre-approval outcome refuses");
        assert_eq!(err.code, "FMINE-REF-108");
        assert!(err.what.contains("INV-008"));
    }

    #[test]
    fn test_hindcast_binds_revisions_even_when_the_rule_choices_stay_the_same() {
        let original = benchmark_five_cases().remove(0);
        let before = evaluate_hindcast_suite(std::slice::from_ref(&original)).unwrap();
        let mut revised = original.clone();
        revised.h7a_hurdle_irr = q(16, 100);
        let after = evaluate_hindcast_suite(&[revised]).unwrap();
        assert_eq!(
            before.results[0].h7a_decision,
            after.results[0].h7a_decision
        );
        assert_ne!(before.digest, after.digest, "the changed hurdle must bind");
        assert_ne!(
            before.results[0].approval_digest,
            after.results[0].approval_digest
        );
        assert_eq!(
            before.results[0].outcome_digest,
            after.results[0].outcome_digest
        );

        let original_digest = before.results[0].approval_digest;
        let mut changed = original.clone();
        changed.approval_evidence[0].evidence_class = EvidenceClass::Assumed;
        assert_ne!(freeze_approval(&changed).unwrap().digest(), original_digest);
        changed = original.clone();
        changed.approval_anchor = Anchor::new([("helm-capex-hindcast", 11)]).unwrap();
        assert_ne!(freeze_approval(&changed).unwrap().digest(), original_digest);
        changed = original.clone();
        changed.scenarios[1].scenario_id.push_str("-revised");
        assert_ne!(freeze_approval(&changed).unwrap().digest(), original_digest);

        changed = original.clone();
        changed.realized_outcome.realized_stamp = mk_stamp(
            date_ns(2022, 2, 24),
            date_ns(2026, 2, 19),
            date_ns(2026, 2, 20),
            "helm-capex-hindcast",
            20,
        );
        let after = evaluate_hindcast_suite(&[changed]).unwrap();
        assert_eq!(
            before.results[0].h7d_advantage_per_share_usd,
            after.results[0].h7d_advantage_per_share_usd
        );
        assert_eq!(
            before.results[0].approval_digest,
            after.results[0].approval_digest
        );
        assert_ne!(
            before.results[0].outcome_digest,
            after.results[0].outcome_digest
        );
        assert_ne!(before.digest, after.digest);
    }

    #[test]
    fn test_hindcast_freezes_without_an_outcome_and_reuses_rule_choices() {
        let original = benchmark_five_cases().remove(0);
        let snapshot = ApprovalSnapshot {
            case_id: original.case_id.clone(),
            project_name: original.project_name.clone(),
            approval_date: original.approval_date,
            approval_as_of_ns: original.approval_as_of_ns,
            approval_anchor: original.approval_anchor.clone(),
            physical_horizon: original.physical_horizon,
            is_synthetic_scenario_grid: original.is_synthetic_scenario_grid,
            approval_evidence: original.approval_evidence.clone(),
            baseline_capex_usd_m: original.baseline_capex_usd_m,
            shares_millions: original.shares_millions,
            h7a_static_npv_usd_m: original.h7a_static_npv_usd_m,
            h7a_static_irr: original.h7a_static_irr,
            h7a_hurdle_irr: original.h7a_hurdle_irr,
            scenarios: original.scenarios.clone(),
            cvar_tail_mass: original.cvar_tail_mass,
            max_cvar_loss_per_share_usd: original.max_cvar_loss_per_share_usd,
            realized_outcome: (),
        };
        let frozen = freeze_approval(&snapshot).unwrap();
        assert_eq!(
            frozen.decisions(),
            (
                ApprovalAction::ApproveFull,
                ApprovalAction::StageWithRecourse
            )
        );
        assert_eq!(
            frozen.action_metrics()[1].cvar_loss_per_share_usd,
            q(12, 100)
        );
        assert_eq!(frozen.approval_as_of_ns(), original.approval_as_of_ns);
        assert!(frozen.is_synthetic_scenario_grid());
        assert_eq!(frozen.admitted_evidence_ids(), ["E631-feb2022-est"]);

        let score = frozen.score(&original.realized_outcome).unwrap();
        let suite = evaluate_hindcast_suite(std::slice::from_ref(&original)).unwrap();
        assert_eq!(score, suite.results[0]);
        let mut outcome = original.realized_outcome;
        outcome.realized_staged_recourse_delta_nav_usd_m = q(-550, 1);
        let revised = frozen.score(&outcome).unwrap();
        assert_eq!(score.approval_digest, revised.approval_digest);
        assert_ne!(score.outcome_digest, revised.outcome_digest);
        assert_eq!(revised.h7d_decision, score.h7d_decision);
        assert_eq!(revised.h7d_advantage_per_share_usd, q(-1, 20));

        let mut permuted = snapshot;
        permuted.scenarios.reverse();
        outcome.outcome_evidence_ids.reverse();
        assert_eq!(freeze_approval(&permuted).unwrap(), frozen);
        assert_eq!(frozen.score(&outcome).unwrap(), revised);
    }

    #[test]
    fn test_hindcast_refuses_a_stale_approval_date_and_a_retrospectively_released_outcome() {
        let original = benchmark_five_cases().remove(0);
        let mut stale_date = original.clone();
        stale_date.approval_as_of_ns += DAY_NS;
        let result = evaluate_hindcast_suite(&[stale_date]);
        assert_eq!(result.unwrap_err().code, "FMINE-REF-108");

        let mut wrong_outcome_day = original.clone();
        wrong_outcome_day.realized_outcome.realized_date = CivilDate::new(2026, 2, 20).unwrap();
        let result = evaluate_hindcast_suite(&[wrong_outcome_day]);
        assert_eq!(result.unwrap_err().code, "FMINE-REF-108");

        let mut stale_outcome = original;
        let cut = stale_outcome.approval_as_of_ns;
        stale_outcome.realized_outcome.realized_date = CivilDate::new(2022, 2, 24).unwrap();
        stale_outcome.realized_outcome.realized_stamp =
            mk_stamp(cut - DAY_NS, cut, cut + DAY_NS, "helm-capex-hindcast", 20);
        let result = evaluate_hindcast_suite(&[stale_outcome]);
        assert_eq!(result.unwrap_err().code, "FMINE-REF-108");
    }

    #[test]
    fn test_hindcast_exact_cvar_matches_sorted_tail_weighted_average() {
        // 3 scenarios: loss 10 (w=1/10), loss 4 (w=3/10), loss -2 (w=6/10).
        // For alpha = 1/4 (0.25): takes 0.10 at loss 10 + 0.15 at loss 4 -> (1.0 + 0.6) / 0.25 = 6.4 = 32/5 exact.
        let cvar = exact_weighted_cvar(
            &[
                (q(6, 10), q(-2, 1)),
                (q(1, 10), q(10, 1)),
                (q(3, 10), q(4, 1)),
            ],
            q(1, 4),
        )
        .expect("exact CVaR");
        assert_eq!(cvar, q(32, 5));
    }

    #[test]
    fn test_hindcast_refuses_bad_weights_uncommitted_anchor_and_too_few_cases() {
        let cases = benchmark_five_cases();
        // GATE-CAP-04 refuses fewer than 5 cases.
        let too_few = evaluate_gate_cap_04_hindcast(&cases[..4]).expect_err("< 5 cases refuses");
        assert_eq!(too_few.code, "FMINE-REF-201");

        // Uncommitted evidence sequence at approval anchor refuses FMINE-REF-108.
        let mut uncommitted = cases[0].clone();
        uncommitted.approval_anchor =
            Anchor::new([("helm-capex-hindcast".to_string(), 9)]).expect("anchor");
        assert_eq!(
            evaluate_hindcast_suite(&[uncommitted]).unwrap_err().code,
            "FMINE-REF-108"
        );

        // Weights not summing to 1 refuse FMINE-REF-109.
        let mut bad_w = cases[0].clone();
        bad_w.scenarios[0].weight = q(1, 10);
        assert_eq!(
            evaluate_hindcast_suite(&[bad_w]).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
}
