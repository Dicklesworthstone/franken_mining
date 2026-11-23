//! DESIGN (BD-176/181/225): source-bound plant scenarios become explicit cash-margin decisions.
//! The physical, valuation and risk kernels retain ownership of their respective calculations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::valuation::{self, comparison, worksheet};

use crate::{Envelope, NextAction, json_string, worksheet_input};

pub const USAGE: &str = "decide production --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --terms <valuation.toml> | decide production --manifest <comparison.toml>";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.production-valuation.v1","command":"decide production --input <production.toml> --grinding <grinding.toml> --recovery <recovery.toml> --terms <valuation.toml>","source":"Recomputes the complete source-bound plant production receipt from the three physical worksheets; retains all four submitted worksheets and their SHA-256 digests","valuation":"Explicit immediate_one_period_cash_margin; USD only, no inferred project NPV, discounting, taxes, sales settlement or stock movements","fields":["schema","model_key","question","valuation_basis","action_id","action_label","boundary","circuit","material_domain","dry_basis","element","start_ns","end_ns","commercial_support","physical_to_commercial_basis","monetary_rounding"],"scenario_fields":["id","weight","payable_fraction","usd_per_kg","variable_usd_per_dry_t","fixed_usd","royalty_fraction","do_nothing_usd"],"population":"Every production scenario exactly once with the same exact probability; no marginal averaging, normalization or missing-state substitution","arithmetic":"Same-scenario recovered kg times explicit payability times USD/kg, less the declared revenue-linked royalty, dry-tonnage cost and fixed cost; the do-nothing payoff is separately declared for every state","rounding":"Explicit nearest_ties_even_0_0001_usd at the final decision Money boundary; unrounded rational results and signed rounding adjustments remain visible","unknowns":"At least one explicit [[uu]] row with id, description, trigger and positive downside_usd_m; source unknowns and evidence remain inherited","output":"Two-action DecisionCard computed by the existing exact evaluator, including expected payoff, quantiles, signed-loss CVaR95, same-state regret and ties","evidence":"Declared commercial terms remain Assumed, Internal and uncalibrated; physical model results do not become measured financial performance or execution authority","example":"tests/decisions/production_valuation.toml","comparison":{"command":"decide production --manifest <comparison.toml>","schema":"fmine.production-comparison.v1","fields":["schema","model_key","question","shared_scenario_basis","boundary","circuit","material_domain","dry_basis","element","start_ns","end_ns","commercial_support"],"alternative_fields":["id","input","grinding","recovery","terms"],"information_basis":"Optional top-level information_basis must equal before_action_same_payoffs_explicit_delay_cost whenever information rows exist. The source declares that the test is observed before acting and that the same operating payoffs remain valid after its delay; no shifted period or lost production is inferred.","information_option_fields":["id","label","test_cost","delay_cost","delay_days"],"signal_fields":["option","id"],"likelihood_fields":["option","state","signal","probability"],"information_options":"Optional [[information_option]], [[signal]] and [[likelihood]] tables reuse the standalone decision grammar. Costs are nonnegative quoted USD decimals with at most four fractional digits in the immediate cash-margin basis; delay_days is an unquoted nonnegative u32 integer and never determines a cost. Every original state and declared signal needs exactly one explicit conditional probability, including 0/1; fractions must sum to one per state. The exact evaluator retains every signal posterior, action tie, EVSI, net VOI, fixed-policy payoff risk, losing test and complete likelihood matrix. Buy at most one test; no combined tests or calibrated test accuracy is inferred.","information_example":"tests/decisions/production_testwork.toml","downside_limit":{"section":"Optional single [[downside_limit]]","fields":["floor_usd","max_breach_probability"],"numbers":"Exact decimal or n/d strings; floor exactly representable in 0.0001 USD; probability in [0,1]","event":"Realized money-rounded cash margin, less test and delay costs once for information policies, strictly below floor; equality at the floor is not a breach and equality at the probability limit is eligible","selection":"Preserves nominal ranking, eligible winners, every action including do-nothing, every fixed one-test policy and expected-value cost of the limit. Policy screening integrates realized state/signal outcomes, not posterior means; signal policies are not reoptimized. No eligible choice means no fallback; conditional uncalibrated probabilities"},"limits":"Two to sixteen complete alternatives; manifest at most 64 KiB; source paths are relative to the workspace root, not the manifest directory","admission":"Each id must match its terms action_id. All alternatives share the declared physical/commercial support, complete state ids, exact weights, commercial bridge, rounding rule and unrounded do-nothing cash. Shared scenario meaning is explicitly assumed, not inferred from labels.","output":"One joint DecisionCard recomputes regret across all operating alternatives and the common do-nothing action; every original source and calculation is retained","example":"tests/decisions/production_comparison.toml"}}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Single {
        input: PathBuf,
        grinding: PathBuf,
        recovery: PathBuf,
        terms: PathBuf,
    },
    Manifest(PathBuf),
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
        if !matches!(
            flag,
            "--input" | "--grinding" | "--recovery" | "--terms" | "--manifest"
        ) {
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
    if let Some(manifest) = fields.get("--manifest") {
        if fields.len() != 1 {
            return Err(
                "decide production: --manifest cannot be combined with individual source flags"
                    .into(),
            );
        }
        return Ok(Command::Manifest(PathBuf::from(*manifest)));
    }
    let path = |flag| {
        fields
            .get(flag)
            .map(|value| PathBuf::from(*value))
            .ok_or_else(|| format!("decide production requires {flag}; {USAGE}"))
    };
    Ok(Command::Single {
        input: path("--input")?,
        grinding: path("--grinding")?,
        recovery: path("--recovery")?,
        terms: path("--terms")?,
    })
}

fn single(
    input: &Path,
    grinding: &Path,
    recovery: &Path,
    terms: &Path,
    root: &Path,
) -> Result<Envelope, Refusal> {
    let production = crate::plant::production::load(input, grinding, recovery, root)?;
    let (text, source) = worksheet_input::read(
        &root.join(terms),
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
}

fn compare(path: &Path, root: &Path) -> Result<Envelope, Refusal> {
    let (text, source) = worksheet_input::read(
        &root.join(path),
        "production comparison manifest",
        comparison::worksheet::MAX_BYTES,
    )?;
    let manifest = comparison::worksheet::parse(&text, &source)?;
    let mut alternatives = Vec::with_capacity(manifest.sources().len());
    let mut snapshots = Vec::with_capacity(manifest.sources().len());
    for bundle in manifest.sources() {
        let production = crate::plant::production::load(
            Path::new(&bundle.input),
            Path::new(&bundle.grinding),
            Path::new(&bundle.recovery),
            root,
        )?;
        let (terms, terms_source) = worksheet_input::read(
            &root.join(&bundle.terms),
            "production valuation terms",
            worksheet::MAX_BYTES,
        )?;
        let receipt = worksheet::evaluate(&terms, &terms_source, production.receipt())?;
        if receipt.terms().value.action_id != bundle.id {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "production comparison: terms action_id does not match alternative {}",
                    bundle.id
                ),
            ));
        }
        snapshots.push(format!(
            "{{\"action_id\":{},\"production\":{},\"terms\":{{\"source\":{},\"submitted_worksheet\":{}}}}}",
            json_string(&bundle.id),
            production.to_json(),
            worksheet_input::source_json(&terms_source, terms.len()),
            json_string(&terms),
        ));
        alternatives.push(receipt);
    }
    let receipt = manifest.evaluate(&alternatives)?;
    let payload = format!(
        "{{\"schema\":\"fmine.production-comparison-command.v1\",\"mode\":\"declared_alternative_cash_margins\",\"manifest\":{{\"source\":{},\"submitted_worksheet\":{}}},\"alternatives\":[{}],\"comparison\":{}}}",
        worksheet_input::source_json(&source, text.len()),
        json_string(&text),
        snapshots.join(","),
        receipt.to_json(),
    );
    let mut envelope = Envelope::ok("decide production", payload, vec![]);
    envelope.rendered_markdown = Some(receipt.to_markdown());
    envelope.no_claim = Some(receipt.no_claim());
    if receipt.decision().summary().information.is_some() {
        envelope.warnings.push("Test likelihoods and unchanged post-delay operating payoffs are source assumptions. Exact information values and realized-policy risk remain conditional; no test accuracy, shifted production period or physical authorization is inferred.".into());
    } else {
        envelope.warnings.push("Common scenario meaning is declared, not inferred from matching labels. Operating alternatives are compared individually; no combination of actions, calibrated probability or physical authorization is inferred.".into());
    }
    Ok(envelope)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let result = match command {
        Command::Single {
            input,
            grinding,
            recovery,
            terms,
        } => single(input, grinding, recovery, terms, root),
        Command::Manifest(path) => compare(path, root),
    };
    let mut envelope = result.unwrap_or_else(|refusal| {
        Envelope::refused(
            "decide production",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the required physical scope, scenario coupling and commercial terms",
            )],
        )
    });
    if envelope.no_claim.is_none() {
        envelope.no_claim = Some(match command {
            Command::Single { .. } => valuation::NO_CLAIM,
            Command::Manifest(_) => comparison::NO_CLAIM,
        });
    }
    envelope
}
