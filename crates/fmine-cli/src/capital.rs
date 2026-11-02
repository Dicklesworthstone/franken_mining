//! Source-bound static capital selection over one complete shared scenario population (BD-229).
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
    "\"mode\":\"declared_static_capital_comparison\",",
    "\"input\":\"Regular UTF-8 TOML file, at most 4 MiB; actual bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"currency\",\"valuation_basis\",\"budget\",\"tail_mass\",\"payoff_convention\",\"reference_class_basis\",\"max_tail_loss (optional)\"],",
    "\"tables\":{\"state\":[\"id\",\"weight\"],\"candidate\":[\"id\",\"label\",\"cost\",\"reference_class_basis\",\"exclusion_group (optional)\"],\"payoff\":[\"candidate\",\"state\",\"value\"],\"requires\":[\"candidate\",\"prerequisite\"],\"uu\":[\"id\",\"description\",\"trigger\",\"downside_usd_m\"],\"funding (optional)\":[\"cash_period\",\"cash_basis\",\"minimum_cash\"],\"available_cash\":[\"state\",\"value\"],\"cash_requirement\":[\"candidate\",\"state\",\"value\"],\"funding_schedule (optional, excludes funding)\":[\"cash_basis\",\"opening_date\"],\"cash_period\":[\"id\",\"cash_period\",\"minimum_cash\"],\"opening_cash\":[\"state\",\"value\"],\"baseline_cash_flow\":[\"period\",\"state\",\"value\"],\"candidate_cash_flow\":[\"period\",\"candidate\",\"state\",\"value\"],\"equity_basis (optional)\":[\"entity\",\"share_class\",\"shares_per_unit\",\"initial_share_units\",\"max_tail_loss_per_share (optional)\"],\"baseline_equity_nav\":[\"state\",\"value\"],\"candidate_share_change\":[\"candidate\",\"state\",\"value\"]},",
    "\"scope\":\"At most eight candidates, 256 shared states and 16 cash periods; every subset is examined. Scheduled funding additionally requires 2^candidates * states * (periods+1) <= 65536 checkpoints\",",
    "\"payoffs\":\"Use additive_net_delta_nav without equity, or additive_equity_nav_with_share_changes with equity_basis. Equity payoffs are total equity NAV changes: buyback cash paid including fees is negative; issuance adds net proceeds. Do not reuse standalone per-share accretion as NAV. Costs and cash constrain funding and are not deducted again\",",
    "\"risk\":\"Exact signed-loss CVaR over the declared tail mass of whole-bundle shared-state outcomes. Equity mode ranks ending NAV per actual share minus baseline NAV per actual share, with exact USD/share quantiles and regret. Use equity_basis.max_tail_loss_per_share; the USD max_tail_loss is forbidden. Total-USD statistics remain separate\",\"equity\":\"Declare one entity and share class, positive initial_share_units and shares_per_unit, baseline NAV for every state, and signed share changes for every candidate/state including zeros. Share quantities, scale and per-share cap require exact quoted n/d strings. Negative changes retire shares, positive changes issue shares. Compute each whole bundle before dividing; preserve NAV/share-change pairing by state. Nonpositive ending shares reject the bundle with undefined ratios. All equity inputs and rejected ledgers remain source-bound; equity can combine with either funding model\",",
    "\"selection\":\"Conditional exact co-winners, rejected bundles, hold-cash, budget slack and the cost of the declared tail-loss limit\",",
    "\"evidence\":\"Declared assumptions remain Assumed, Internal and uncalibrated; no fitted reference-class adjustment, selection-frequency estimate or execution authority\",",
    "\"example\":\"tests/capital/gb30.toml\",",
    "\"liquidity\":\"Declare either aggregate funding or a complete funding_schedule. Scheduled periods are contiguous inclusive YYYY-MM-DD/YYYY-MM-DD ranges starting at opening_date. Opening cash is explicit in every state; signed baseline and candidate flows require all period/state and period/candidate/state rows, including zeros. Positive flows are receipts, negative flows expenditures; baseline excludes the selected program. Balances carry forward exactly. The first floor applies at opening and each period floor at its close. Every breach remains a rejection even after later recovery; financing may make a nonempty program feasible when hold fails. No intra-period balances or adaptive recourse are inferred\",",
    "\"liquidity_example\":\"tests/capital/liquidity.toml\",\"cash_schedule_example\":\"tests/capital/cash_schedule.toml\",\"equity_example\":\"tests/capital/equity.toml\"",
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
        let inputs = fmine_capital::model::parse(
            text,
            SourceAnchor::new(source_path, digest.0, "declared static capital program"),
        )?;
        let evaluated = fmine_capital::evaluate(&inputs)?;
        let payload = format!(
            "{{\"mode\":\"declared_static_capital_comparison\",\"source\":{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{}}},\"program\":{}}}",
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
