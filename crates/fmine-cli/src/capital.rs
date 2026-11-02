//! Source-bound static and two-stage capital comparison over shared scenarios (BD-229).
//! Determinism: Exact (bounded model input and checked rational portfolio calculations).

use std::io::Read as _;
use std::path::Path;

use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};

use crate::{Envelope, NextAction, escape_json};

const MAX_MODEL_BYTES: u64 = 4 * 1024 * 1024;
pub(super) const USAGE: &str = "decide program <model-file>";
pub(super) const ROBOT_DOCS_JSON: &str = concat!(
    "{",
    "\"schema\":\"fmine.capital-program.v1\",",
    "\"modes\":[\"declared_static_capital_comparison\",\"declared_two_stage_capital_comparison\"],",
    "\"input\":\"Regular UTF-8 TOML file, at most 4 MiB; actual bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"currency\",\"valuation_basis\",\"budget\",\"tail_mass\",\"payoff_convention\",\"reference_class_basis\",\"max_tail_loss (optional)\"],",
    "\"tables\":{\"state\":[\"id\",\"weight\"],\"candidate\":[\"id\",\"label\",\"cost\",\"reference_class_basis\",\"exclusion_group (optional)\"],\"payoff\":[\"candidate\",\"state\",\"value\"],\"requires\":[\"candidate\",\"prerequisite\"],\"uu\":[\"id\",\"description\",\"trigger\",\"downside_usd_m\"],\"funding (optional)\":[\"cash_period\",\"cash_basis\",\"minimum_cash\"],\"available_cash\":[\"state\",\"value\"],\"cash_requirement\":[\"candidate\",\"state\",\"value\"],\"funding_schedule (optional, excludes funding)\":[\"cash_basis\",\"opening_date\"],\"cash_period\":[\"id\",\"cash_period\",\"minimum_cash\"],\"opening_cash\":[\"state\",\"value\"],\"baseline_cash_flow\":[\"period\",\"state\",\"value\"],\"candidate_cash_flow\":[\"period\",\"candidate\",\"state\",\"value\"],\"equity_basis (optional)\":[\"entity\",\"share_class\",\"shares_per_unit\",\"initial_share_units\",\"max_tail_loss_per_share (optional)\"],\"baseline_equity_nav\":[\"state\",\"value\"],\"candidate_share_change\":[\"candidate\",\"state\",\"value\"]},",
    "\"scope\":\"At most eight candidates, 256 shared states and 16 cash periods; every subset is examined. Scheduled funding additionally requires 2^candidates * states * (periods+1) <= 65536 checkpoints. Entity funding admits at most four entities and 64 transfers; with E entities, P periods, T transfers and D distribution edges, work per state is E*(2*P+1)+T+D*P+P. Total entity rows across all subsets and policies must not exceed 65536. Two-stage policies require at least one initial and one recourse candidate, 2..16 cash periods, at most four observation classes and 256 complete policies; combined static and policy cash work must not exceed 65536\",",
    "\"payoffs\":\"Use additive_net_delta_nav without equity, or additive_equity_nav_with_share_changes with equity_basis. Equity payoffs are total equity NAV changes: buyback cash paid including fees is negative; issuance adds net proceeds. Do not reuse standalone per-share accretion as NAV. Costs and cash constrain funding and are not deducted again\",",
    "\"risk\":\"Exact signed-loss CVaR over the declared tail mass of whole-bundle shared-state outcomes. Equity mode ranks ending NAV per actual share minus baseline NAV per actual share, with exact USD/share quantiles and regret. Use equity_basis.max_tail_loss_per_share; the USD max_tail_loss is forbidden. Total-USD statistics remain separate\",\"equity\":\"Declare one entity and share class, positive initial_share_units and shares_per_unit, baseline NAV for every state, and signed share changes for every candidate/state including zeros. Share quantities, scale and per-share cap require exact quoted n/d strings. Negative changes retire shares, positive changes issue shares. Compute each whole bundle before dividing; preserve NAV/share-change pairing by state. Nonpositive ending shares reject the bundle with undefined ratios. All equity inputs and rejected ledgers remain source-bound; equity can combine with any declared funding model. With entity_funding, equity_basis.entity must equal its objective_entity identifier\",",
    "\"selection\":\"Conditional exact co-winners, rejected bundles, hold-cash, budget slack and the cost of the declared tail-loss limit\",",
    "\"evidence\":\"Declared assumptions remain Assumed, Internal and uncalibrated; no fitted reference-class adjustment, selection-frequency estimate or execution authority\",",
    "\"example\":\"tests/capital/gb30.toml\",",
    "\"liquidity\":\"Choose at most one of aggregate funding, funding_schedule or entity_funding. Scheduled periods are contiguous inclusive YYYY-MM-DD/YYYY-MM-DD ranges starting at opening_date. Opening cash is explicit in every state; signed baseline and candidate flows require all period/state and period/candidate/state rows, including zeros. Positive flows are receipts, negative flows expenditures; baseline excludes the selected program. Balances carry forward exactly. The first floor applies at opening and each period floor at its close. Every breach remains a rejection even after later recovery; financing may make a nonempty program feasible when hold fails. No intra-period balances or adaptive recourse are inferred\",\"entity_funding_example\":\"tests/capital/entity_funding.toml\",\"entity_tables\":{\"entity_funding\":[\"cash_basis\",\"opening_date\",\"objective_entity\"],\"entity\":[\"id\",\"label\"],\"entity_period\":[\"id\",\"cash_period\"],\"entity_floor\":[\"entity\",\"period\",\"value\"],\"entity_opening_cash\":[\"entity\",\"state\",\"value\"],\"entity_baseline_cash_flow\":[\"entity\",\"period\",\"state\",\"value\"],\"entity_candidate_cash_flow\":[\"entity\",\"period\",\"candidate\",\"state\",\"value\"],\"entity_transfer\":[\"id\",\"from_entity\",\"to_entity\",\"period\",\"basis\",\"candidate (optional)\"],\"entity_transfer_amount\":[\"transfer\",\"state\",\"value\"],\"entity_distribution\":[\"entity\",\"parent_entity\",\"parent_share\",\"basis\"],\"entity_distribution_gate\":[\"entity\",\"period\",\"state\",\"allowed (quoted true or false)\",\"maximum_distribution\"]},\"entity_liquidity\":\"Keep complete entity cash matrices including zeros. Apply native external flows and fixed transfers, check every entity floor, distribute child-first, then check closing floors and carry cash. Each distributing entity has one declared parent in an acyclic graph and a quoted parent_share n/d in [0,1]. Gross distribution is zero when blocked, otherwise min(cap, positive cash above floor); its exact parent fraction stays inside the graph and the remainder leaves it. Every distributing entity/period/state needs an explicit permission and cap. Opening and pre-distribution failures remain binding; put a receipt in an earlier period if it must fund later spending. Fixed transfers are not clipped. Record external partner contributions as native cash and parent calls as internal transfers without counting them twice. No top-up, dilution, changing ownership or debt-service test is inferred. All ledger rows, including inactive transfers and blocked distributions, remain in reports with exact period conservation\",",
    "\"liquidity_example\":\"tests/capital/liquidity.toml\",\"cash_schedule_example\":\"tests/capital/cash_schedule.toml\",\"equity_example\":\"tests/capital/equity.toml\",\"recourse_example\":\"tests/capital/recourse.toml\",\"recourse_tables\":{\"recourse\":[\"information_candidate\",\"observation_date\",\"information_basis\"],\"candidate_stage\":[\"candidate\",\"stage (initial or recourse)\"],\"state_observation\":[\"state\",\"observation\"]},\"recourse\":\"Optional explicit two-stage mode with funding_schedule or entity_funding. Map every candidate and shared state. The initial information candidate must have positive cost, negative net NAV in every state and negative cumulative native external cash before observation_date, which starts a noninitial cash period. Recourse native cash and activated transfers before observation must be explicitly zero. Internal transfers alone do not pay for information. Without the gate one common continuation applies; after purchasing it continuations depend only on observation classes. All initial effects persist, including after abandonment. Budget, prerequisites, exclusions, cash and shares apply on each realized path; CVaR uses the full unconditional policy distribution. The report retains all policies and the static comparison. Net adaptive value is separately the best policy mean minus the best static mean under the same objective/risk rule, with explicit unavailable comparisons; never add it to NAV again\"",
    "}",
);

pub(super) fn execute(model: &Path, root: &Path) -> Envelope {
    let path = root.join(model);
    let unreadable = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("capital program {}: {why}", path.display()),
        )
    };
    let run = || -> Result<Envelope, Refusal> {
        let source_path = path
            .to_str()
            .ok_or_else(|| unreadable("source path must be UTF-8".into()))?;
        let metadata = std::fs::metadata(&path).map_err(|e| unreadable(e.to_string()))?;
        if !metadata.is_file() || metadata.len() > MAX_MODEL_BYTES {
            return Err(unreadable(
                "expected a regular file of at most 4 MiB".into(),
            ));
        }
        let file = std::fs::File::open(&path).map_err(|e| unreadable(e.to_string()))?;
        let mut bytes = Vec::new();
        file.take(MAX_MODEL_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| unreadable(e.to_string()))?;
        if bytes.len() as u64 > MAX_MODEL_BYTES {
            return Err(unreadable("model exceeds 4 MiB".into()));
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| unreadable(e.to_string()))?;
        let digest = sha256(&bytes);
        let evaluated = fmine_capital::model::evaluate(
            text,
            SourceAnchor::new(source_path, digest.0, "declared static capital program"),
        )?;
        let payload = format!(
            "{{\"mode\":\"{}\",\"source\":{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{}}},\"program\":{}}}",
            evaluated.mode(),
            escape_json(source_path),
            digest.to_hex(),
            bytes.len(),
            evaluated.to_json(),
        );
        let mut envelope = Envelope::ok("decide program", payload, Vec::new());
        envelope.no_claim = Some(evaluated.no_claim());
        envelope.rendered_markdown = Some(evaluated.to_markdown());
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "decide program",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the capital-program model requirements",
            )],
        )
    })
}
