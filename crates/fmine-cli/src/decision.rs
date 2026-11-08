//! Draft monetary comparisons and information options from explicit scenario assumptions (BD-225/227).
//! Determinism: Exact (bounded model bytes and the exact finite-scenario evaluator).

use std::path::Path;

use fmine_decide::card;
use fmine_evidence::Refusal;

use crate::{Envelope, NextAction, worksheet_input};

pub(crate) const ROBOT_DOCS_JSON: &str = concat!(
    "{\"schema\":\"fmine.decision-model.v1\",\"mode\":\"declared_model_comparison\",",
    "\"input\":\"Regular UTF-8 TOML file, at most 4 MiB; actual bytes are SHA-256 bound\",",
    "\"fields\":[\"schema\",\"model_key\",\"question\",\"currency\",\"valuation_basis\"],",
    "\"tables\":{\"state\":[\"id\",\"weight\"],\"action\":[\"id\",\"label\",\"role\"],",
    "\"payoff\":[\"action\",\"state\",\"value\"],\"uu\":[\"id\",\"description\",\"trigger\",\"downside_usd_m\"],",
    "\"information_option\":[\"id\",\"label\",\"test_cost\",\"delay_cost\",\"delay_days\"],",
    "\"signal\":[\"option\",\"id\"],\"likelihood\":[\"option\",\"state\",\"signal\",\"probability\"],\"robustness\":[\"method\",\"radius\",\"metric_basis\",\"radius_basis\"],\"transport_distance\":[\"from\",\"to\",\"value\"]},",
    "\"weights\":\"Quoted positive rational n/d; exact sum one\",",
    "\"actions\":\"At least two; roles action or do_nothing, exactly one do_nothing; complete common states\",",
    "\"money\":\"Quoted major-currency amounts, at most four decimal places; no implicit FX or discounting\",",
    "\"unknowns\":\"At least one named UU with positive finite USD-million exposure and trigger; outside priced states\",",
    "\"output\":\"Exact weighted mean, P10/P50/P90, signed-loss CVaR95, same-state regret and all co-winners\",",
    "\"information\":\"Optional single-test policies; exact EVPI, EVSI and net value after explicit nonnegative test/delay costs in model currency; delay_days is an integer duration, not a discount rule\",",
    "\"likelihoods\":\"Quoted P(signal|state) in [0,1]; explicit complete state-by-signal matrix including zeros; each state sums exactly to one\",",
    "\"method\":\"Exact finite enumeration; Monte Carlo standard error not applicable; assumed probabilities remain uncalibrated\",\"robustness\":\"Optional single wasserstein_1 declaration: quoted nonnegative rational radius and dimensionless distances, explicit metric/radius bases, complete proper metric including zero diagonal, at most 64 states and 256 actions. Reports worst mean and worst expected regret of original act-now actions with exact transport certificates. With information options, also compares fixed displayed signal policies by worst mean: only state weights vary, likelihoods and posterior tie choices remain fixed, and test/delay costs are paid once. Conditional state means are not signal outcomes; no policy reoptimization, test-accuracy stress, cross-test regret, robust CVaR or calibrated confidence claim\",",
    "\"evidence\":\"Assumed, Internal, uncalibrated; no execution authority or decision-ledger append\",",
    "\"example\":\"tests/decisions/expansion.toml\",\"information_example\":\"tests/decisions/metallurgical_test.toml\",\"robustness_example\":\"tests/decisions/wasserstein.toml\"}"
);

pub(crate) fn execute(model: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read_model(
            &root.join(model),
            "decision model",
            "declared decision model",
        )?;
        let inputs = card::model::parse(&text, source.clone())?;
        let evaluated = card::evaluate(&inputs)?;
        let payload = format!(
            "{{\"mode\":\"declared_model_comparison\",\"source\":{},\"card\":{}}}",
            worksheet_input::source_json(&source, text.len()),
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
