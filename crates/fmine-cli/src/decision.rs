//! Draft monetary comparisons and information options from explicit scenario assumptions (BD-225/227).
//! Determinism: Exact (bounded model bytes and the exact finite-scenario evaluator).

use std::io::Read as _;
use std::path::Path;

use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_decide::card;
use fmine_evidence::{Refusal, RefusalCode};

use crate::{Envelope, NextAction, escape_json};

const MAX_MODEL_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.decision-model.v1\",\"mode\":\"declared_model_comparison\",",
    "\"input\":\"Regular UTF-8 TOML file, at most 4 MiB; actual bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"currency\",\"valuation_basis\"],",
    "\"tables\":{\"state\":[\"id\",\"weight\"],\"action\":[\"id\",\"label\",\"role\"],",
    "\"payoff\":[\"action\",\"state\",\"value\"],\"uu\":[\"id\",\"description\",\"trigger\",\"downside_usd_m\"],",
    "\"information_option\":[\"id\",\"label\",\"test_cost\",\"delay_cost\",\"delay_days\"],",
    "\"signal\":[\"option\",\"id\"],\"likelihood\":[\"option\",\"state\",\"signal\",\"probability\"]},",
    "\"weights\":\"Quoted positive rational n/d; exact sum one\",",
    "\"actions\":\"At least two; roles action or do_nothing, exactly one do_nothing; complete common states\",",
    "\"money\":\"Quoted major-currency amounts, at most four decimal places; no implicit FX or discounting\",",
    "\"unknowns\":\"At least one named UU with positive finite USD-million exposure and trigger; outside priced states\",",
    "\"output\":\"Exact weighted mean, P10/P50/P90, signed-loss CVaR95, same-state regret and all co-winners\",",
    "\"information\":\"Optional single-test policies; exact EVPI, EVSI and net value after explicit nonnegative test/delay costs in model currency; delay_days is an integer duration, not a discount rule\",",
    "\"likelihoods\":\"Quoted P(signal|state) in [0,1]; explicit complete state-by-signal matrix including zeros; each state sums exactly to one\",",
    "\"method\":\"Exact finite enumeration; Monte Carlo standard error not applicable; assumed probabilities remain uncalibrated\",",
    "\"evidence\":\"Assumed, Internal, uncalibrated; no execution authority or decision-ledger append\",",
    "\"example\":\"tests/decisions/expansion.toml\",\"information_example\":\"tests/decisions/metallurgical_test.toml\"}"
);

pub(crate) fn execute(model: &Path, root: &Path) -> Envelope {
    let path = root.join(model);
    let unreadable = |why: String| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("decision model {}: {why}", path.display()),
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
        let inputs = card::model::parse(
            text,
            SourceAnchor::new(source_path, digest.0, "declared decision model"),
        )?;
        let evaluated = card::evaluate(&inputs)?;
        let payload = format!(
            "{{\"mode\":\"declared_model_comparison\",\"source\":{{\"path\":\"{}\",\"sha256\":\"{}\",\"bytes\":{}}},\"card\":{}}}",
            escape_json(source_path),
            digest.to_hex(),
            bytes.len(),
            evaluated.to_json(),
        );
        let mut envelope = Envelope::ok("decide card", payload, Vec::new());
        envelope.no_claim = Some(card::NO_CLAIM);
        envelope.warnings.push(
            "Declared-model inputs are Assumed; state probabilities and test likelihoods are uncalibrated.".into(),
        );
        envelope.rendered_markdown = Some(evaluated.to_markdown());
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "decide card",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the decision-model command and input requirements",
            )],
        )
    })
}
