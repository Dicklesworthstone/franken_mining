//! DESIGN (BD-176/181/225): source-bound plant scenarios become explicit cash-margin decisions.
//! The physical, valuation and risk kernels retain ownership of their respective calculations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_evidence::Refusal;
use fmine_metrics::valuation::{self, worksheet};

use crate::{Envelope, NextAction, json_string, worksheet_input};

pub const USAGE: &str = "decide production --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --terms <valuation.toml>";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.production-valuation.v1","command":"decide production --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --terms <valuation.toml>","source":"Recomputes the complete source-bound plant production receipt from the three physical worksheets; retains all four submitted worksheets and their SHA-256 digests","valuation":"Explicit immediate_one_period_cash_margin; USD only, no inferred project NPV, discounting, taxes, sales settlement or stock movements","fields":["schema","model_key","question","valuation_basis","action_id","action_label","boundary","circuit","material_domain","dry_basis","element","start_ns","end_ns","commercial_support","physical_to_commercial_basis","monetary_rounding"],"scenario_fields":["id","weight","payable_fraction","usd_per_kg","variable_usd_per_dry_t","fixed_usd","royalty_fraction","do_nothing_usd"],"population":"Every production scenario exactly once with the same exact probability; no marginal averaging, normalization or missing-state substitution","arithmetic":"Same-scenario recovered kg times explicit payability times USD/kg, less the declared revenue-linked royalty, dry-tonnage cost and fixed cost; the do-nothing payoff is separately declared for every state","rounding":"Explicit nearest_ties_even_0_0001_usd at the final decision Money boundary; unrounded rational results and signed rounding adjustments remain visible","unknowns":"At least one explicit [[uu]] row with id, description, trigger and positive downside_usd_m; source unknowns and evidence remain inherited","output":"Two-action DecisionCard computed by the existing exact evaluator, including expected payoff, quantiles, signed-loss CVaR95, same-state regret and ties","evidence":"Declared commercial terms remain Assumed, Internal and uncalibrated; physical model results do not become measured financial performance or execution authority","example":"tests/decisions/production_valuation.toml"}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub input: PathBuf,
    pub grinding: PathBuf,
    pub recovery: PathBuf,
    pub terms: PathBuf,
}

/// Shared workspace/output flags are removed by the command router.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("decide")
        || words.get(1).map(String::as_str) != Some("production")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut i = 2;
    while i < words.len() {
        let flag = words[i].as_str();
        if !matches!(flag, "--input" | "--grinding" | "--recovery" | "--terms") {
            return Err(format!(
                "decide production: unsupported option {flag}; {USAGE}"
            ));
        }
        let value = words
            .get(i + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or_else(|| format!("decide production: missing value for {flag}; {USAGE}"))?;
        if fields.insert(flag, value.as_str()).is_some() {
            return Err(format!("decide production: duplicate {flag}"));
        }
        i += 2;
    }
    let path = |flag| {
        fields
            .get(flag)
            .map(|value| PathBuf::from(*value))
            .ok_or_else(|| format!("decide production requires {flag}; {USAGE}"))
    };
    Ok(Command {
        input: path("--input")?,
        grinding: path("--grinding")?,
        recovery: path("--recovery")?,
        terms: path("--terms")?,
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let production = crate::plant::production::load(
            &command.input,
            &command.grinding,
            &command.recovery,
            root,
        )?;
        let (text, source) = worksheet_input::read(
            &root.join(&command.terms),
            "production valuation terms",
            worksheet::MAX_BYTES,
        )?;
        let receipt = worksheet::evaluate(&text, &source, production.receipt())?;
        let payload = format!(
            "{{\"schema\":\"fmine.production-decision-command.v1\",\"mode\":\"declared_immediate_cash_margin\",\"production\":{},\"terms\":{{\"schema\":{},\"source\":{},\"submitted_worksheet\":{}}},\"valuation\":{}}}",
            production.to_json(),
            json_string(worksheet::SCHEMA),
            worksheet_input::source_json(&source, text.len()),
            json_string(&text),
            receipt.to_json(),
        );
        let mut envelope = Envelope::ok("decide production", payload, vec![]);
        envelope.rendered_markdown = Some(receipt.to_markdown());
        envelope.warnings.push(
            "Conditional one-period cash margins use explicit commercial assumptions and uncalibrated scenario probabilities; no actual sales or project NPV is inferred.".into(),
        );
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "decide production",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the required physical scope, scenario coupling and commercial terms",
            )],
        )
    });
    envelope.no_claim = Some(valuation::NO_CLAIM);
    envelope
}
