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
    "{\"schema\":\"fmine.capital-program.v1\",\"mode\":\"declared_static_capital_comparison\",",
    "\"input\":\"Regular UTF-8 TOML file, at most 4 MiB; actual bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"currency\",\"valuation_basis\",\"budget\",\"tail_mass\",\"payoff_convention\",\"reference_class_basis\",\"max_tail_loss (optional)\"],",
    "\"tables\":{\"state\":[\"id\",\"weight\"],\"candidate\":[\"id\",\"label\",\"cost\",\"reference_class_basis\",\"exclusion_group (optional)\"],",
    "\"payoff\":[\"candidate\",\"state\",\"value\"],\"requires\":[\"candidate\",\"prerequisite\"],\"uu\":[\"id\",\"description\",\"trigger\",\"downside_usd_m\"]},",
    "\"scope\":\"At most eight candidates and 256 shared states; every subset is examined\",",
    "\"payoffs\":\"Explicit additive_net_delta_nav convention: costs constrain spending and are not deducted again\",",
    "\"risk\":\"Exact signed-loss CVaR over the declared tail mass of the summed joint-scenario payoffs; never marginal CVaR addition\",",
    "\"selection\":\"Conditional exact co-winners, rejected bundles, hold-cash, budget slack and the cost of the declared tail-loss limit\",",
    "\"evidence\":\"Declared assumptions remain Assumed, Internal and uncalibrated; no fitted reference-class adjustment, selection-frequency estimate or execution authority\",",
    "\"example\":\"tests/capital/gb30.toml\"}"
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
        envelope.no_claim = Some(fmine_capital::NO_CLAIM);
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
