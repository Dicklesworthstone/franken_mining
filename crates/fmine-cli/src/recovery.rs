//! DESIGN (bd-srqc): explicit copper-price scenarios beside the existing source-bound recovery report.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_claim::Claim;
use fmine_core::hash::blake3_hash;
use fmine_evidence::Refusal;
use fmine_metrics::recovery::{
    Model, render,
    sensitivity::{self, CopperPrice, Sensitivity},
};
use fmine_numerics::exact_feas::Rat;
use fmine_public::recovery::RecoveryReport;

use crate::{CliCommand, Envelope, NextAction, OutputFormat, json_string as js};

const MAX_PRICES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PriceInput {
    token: String,
    price: CopperPrice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    issuer: Option<String>,
    through: Option<String>,
    prices: Vec<PriceInput>,
}

/// Strip only the new repeatable option, then reuse the established source-selector parser.
pub(super) fn parse(
    args: &[String],
    root: &Path,
) -> Result<(Options, OutputFormat, PathBuf), String> {
    let mut rest = Vec::new();
    let mut prices = Vec::new();
    let mut names = BTreeSet::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] != "--price" {
            rest.push(args[i].clone());
            i += 1;
            continue;
        }
        i += 1;
        let raw = args.get(i).ok_or("--price requires SCENARIO=USD_PER_LB")?;
        let (scenario, token) = raw
            .split_once('=')
            .ok_or("--price requires SCENARIO=USD_PER_LB")?;
        let (integer, fraction) = token.split_once('.').unwrap_or((token, "0"));
        if token.len() > 64
            || integer.is_empty()
            || fraction.is_empty()
            || !integer
                .bytes()
                .chain(fraction.bytes())
                .all(|c| c.is_ascii_digit())
        {
            return Err("--price requires a nonnegative plain decimal USD/lb value".into());
        }
        let price = CopperPrice {
            scenario: scenario.into(),
            usd_per_lb: Rat::from_decimal_str(token)
                .map_err(|_| "--price exceeds exact decimal capacity")?,
        };
        price.clone().declared_claim().map_err(|r| r.what)?;
        if !names.insert(scenario.to_string()) || prices.len() == MAX_PRICES {
            return Err(format!(
                "--price requires distinct scenario names and at most {MAX_PRICES} scenarios"
            ));
        }
        prices.push(PriceInput {
            token: token.into(),
            price,
        });
        i += 1;
    }
    if prices.is_empty() {
        return Err("at least one explicit --price is required".into());
    }
    // Keep this extension strict even where the legacy router accepts spare positional arguments.
    let (words, _, _) = crate::common_options(&rest, root)?;
    let (fields, positional) = crate::journal_arguments(&words, &["--synthetic"])?;
    if positional != ["recovery", "gap"]
        || fields
            .keys()
            .any(|field| !matches!(field.as_str(), "--synthetic" | "--issuer" | "--through"))
    {
        return Err("recovery price sensitivity accepts only --issuer, --through or --synthetic and repeated --price".into());
    }
    // No --price remains, so the ordinary parser cannot dispatch recursively to this adapter.
    let (command, format, workspace) = crate::parse_args(&rest, root)?;
    let (issuer, through) = match command {
        CliCommand::RecoveryGap { issuer, through } => (Some(issuer), through),
        CliCommand::RecoverySynthetic => (None, None),
        _ => return Err("--price applies only to recovery gap".into()),
    };
    prices.sort_by(|a, b| a.price.scenario.cmp(&b.price.scenario));
    Ok((
        Options {
            issuer,
            through,
            prices,
        },
        format,
        workspace,
    ))
}

fn price_json(value: &CopperPrice) -> String {
    format!(
        "{{\"scenario\":{},\"unit\":\"usd_per_lb\",\"value\":{},\"basis\":\"caller_declared_constant_nominal_price\",\"market_observation\":false}}",
        js(&value.scenario),
        render::rational(value.usd_per_lb)
    )
}

fn value_json(value: &Sensitivity) -> String {
    format!(
        "{{\"subject\":{},\"period\":{},\"ownership\":{},\"model\":{},\"price\":{},\"copper_difference_kt\":{},\"value_difference_usd_m\":{}}}",
        js(&value.subject),
        render::period(value.period),
        render::ownership(value.ownership),
        js(model(value.model)),
        price_json(&value.price),
        render::estimate(value.copper_difference_kt),
        render::estimate(value.value_difference_usd_m)
    )
}

fn model(value: Model) -> &'static str {
    match value {
        Model::ConstantTails => "constant_tails",
        Model::GradeDependentTails => "grade_dependent_tails",
    }
}

fn refused_json(refusal: &Refusal) -> String {
    format!(
        "{{\"code\":{},\"what\":{},\"fix\":{}}}",
        js(refusal.code),
        js(&refusal.what),
        js(refusal.fix)
    )
}

fn quoted(value: Rat) -> String {
    format!("{}/{}", value.num(), value.den())
}

fn report(base: &RecoveryReport, options: &Options) -> Result<Envelope, Refusal> {
    let prices: Vec<_> = options
        .prices
        .iter()
        .map(|p| p.price.clone().declared_claim())
        .collect::<Result<_, _>>()?;
    let price_inputs = prices
        .iter()
        .zip(&options.prices)
        .map(|(claim, input)| {
            let canonical = crate::claim_inspection::canonical_claim_json(
                claim,
                "declared_price_assumption",
                price_json(&claim.value),
            )?;
            Ok(format!(
                "{{\"declared_token\":{},\"claim\":{canonical}}}",
                js(&input.token)
            ))
        })
        .collect::<Result<Vec<_>, Refusal>>()?
        .join(",");
    let mut rows = Vec::new();
    let mut first_refusal = None;
    let mut markdown = format!(
        "# Recovery-gap copper-price sensitivity\n\n{}\n\nSource recovery digest: `{}`. Prices below are declared assumptions in nominal USD per avoirdupois pound. The original full-site calendar quarter is retained.\n\n| Quarter | Model | Scenario | Price USD/lb | Signed USD millions | Rounding enclosure USD millions |\n|---|---|---|---|---|---|\n",
        sensitivity::NOTICE,
        base.digest().to_hex()
    );
    for quarter in &base.quarters {
        for price in &prices {
            for kind in [Model::ConstantTails, Model::GradeDependentTails] {
                let result: Result<Claim<Sensitivity>, Refusal> = match &quarter.outcome {
                    Ok(projections) => sensitivity::assess(
                        match kind {
                            Model::ConstantTails => &projections.constant_tails,
                            Model::GradeDependentTails => &projections.grade_dependent_tails,
                        },
                        price,
                    ),
                    Err(refusal) => Err(refusal.clone()),
                };
                let result = result.and_then(|claim| {
                    let json = crate::claim_inspection::canonical_claim_json(
                        &claim,
                        "hypothetical_gross_metal_value_sensitivity",
                        value_json(&claim.value),
                    )?;
                    Ok((claim, json))
                });
                let outcome = match result {
                    Ok((claim, json)) => {
                        let estimate = claim.value.value_difference_usd_m;
                        let _ = writeln!(
                            markdown,
                            "| {} | {} | {} | {} | {} | [{}, {}] |",
                            quarter.period.code(),
                            model(kind),
                            price.value.scenario,
                            quoted(price.value.usd_per_lb),
                            quoted(estimate.point),
                            quoted(estimate.interval.lo),
                            quoted(estimate.interval.hi)
                        );
                        format!("\"status\":\"computed\",\"claim\":{json},\"refusal\":null")
                    }
                    Err(refusal) => {
                        let _ = writeln!(
                            markdown,
                            "| {} | {} | {} | {} | refused: {} | unavailable |",
                            quarter.period.code(),
                            model(kind),
                            price.value.scenario,
                            quoted(price.value.usd_per_lb),
                            refusal.code
                        );
                        first_refusal.get_or_insert_with(|| refusal.clone());
                        format!(
                            "\"status\":\"refused\",\"claim\":null,\"refusal\":{}",
                            refused_json(&refusal)
                        )
                    }
                };
                rows.push(format!(
                    "{{\"quarter\":{},\"model\":{},\"scenario\":{},{outcome}}}",
                    js(&quarter.period.code()),
                    js(model(kind)),
                    js(&price.value.scenario)
                ));
            }
        }
    }
    let body = format!(
        "\"schema\":\"fmine.recovery.price-sensitivity.v1\",\"rule\":{},\"interpretation\":{},\"base_recovery_digest\":{},\"recovery\":{},\"prices\":[{price_inputs}],\"results\":[{}]",
        js(sensitivity::RULE),
        js(sensitivity::NOTICE),
        js(&base.digest().to_hex()),
        base.to_json(),
        rows.join(",")
    );
    let digest = blake3_hash(body.as_bytes()).to_hex();
    let actions = vec![NextAction::new("fmine doctor", crate::DOCTOR_WHY)];
    let payload = format!("{{\"digest\":{},{} }}", js(&digest), body);
    let mut envelope = match first_refusal {
        Some(refusal) => {
            let mut env = Envelope::refused("recovery gap", refusal, actions);
            env.payload_json = payload;
            env
        }
        None => Envelope::ok("recovery gap", payload, actions),
    };
    let _ = writeln!(
        markdown,
        "\nSensitivity digest: `{digest}`. Exact fractions preserve every signed bound; no price weights, expected value or annualization are inferred.\n\n## Retained source and calculation evidence\n\n```json\n{}\n```",
        envelope.payload_json
    );
    envelope.no_claim = Some(sensitivity::NOTICE);
    envelope.rendered_markdown = Some(markdown);
    Ok(envelope)
}

pub(super) fn execute(options: &Options, workspace: &Path) -> Envelope {
    let result = match &options.issuer {
        Some(issuer) => {
            fmine_public::recovery::run_public(workspace, issuer, options.through.as_deref())
        }
        None => fmine_public::recovery::run_synthetic(),
    }
    .and_then(|base| report(&base, options));
    result.unwrap_or_else(|refusal| {
        Envelope::refused(
            "recovery gap",
            refusal,
            vec![NextAction::new("fmine doctor", crate::DOCTOR_WHY)],
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_synthetic(extra: &[&str]) -> Result<Options, String> {
        let args: Vec<_> = ["recovery", "gap", "--synthetic"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(String::from)
            .collect();
        parse(&args, Path::new(".")).map(|(options, _, _)| options)
    }

    #[test]
    fn declared_scenarios_are_exact_ordered_and_strictly_parsed() {
        let options = parse_synthetic(&["--price", "high=5.50", "--price", "low=4.00"]).unwrap();
        assert_eq!(options.prices[0].price.usd_per_lb, Rat::new(11, 2).unwrap());
        for fields in [
            vec!["--price"],
            vec!["--price", "low=-4"],
            vec!["--price", "low=NaN"],
            vec!["--price", "low=4e1"],
            vec!["--price", "low=4", "--price", "low=5"],
            vec!["--price", "a b=4"],
            vec!["--price", "low=4", "extra"],
            vec!["--price", "low=4", "--site", "mine"],
            vec!["--price", "low=4", "--synthetic"],
            vec!["--price", "low=4", "--format", "md", "--json"],
        ] {
            assert!(parse_synthetic(&fields).is_err(), "{fields:?}");
        }
    }

    #[test]
    fn wrapper_keeps_original_recovery_and_every_price_model_with_stable_digests() {
        let base = fmine_public::recovery::run_synthetic().unwrap();
        let options = parse_synthetic(&["--price", "low=4", "--price", "high=5.5"]).unwrap();
        let a = report(&base, &options).unwrap();
        assert_eq!(a.exit_code, 0);
        assert_eq!(
            a.payload_json.matches("\"status\":\"computed\"").count(),
            4 * base.quarters.len()
        );
        assert!(a.payload_json.contains(&base.to_json()));
        assert!(a.payload_json.contains("\"market_observation\":false"));
        assert!(a.anchor_id.is_none() && a.certificate.is_none());
        let reversed = parse_synthetic(&["--price", "high=5.5", "--price", "low=4"]).unwrap();
        assert_eq!(a, report(&base, &reversed).unwrap());
        let changed = parse_synthetic(&["--price", "low=4", "--price", "high=6"]).unwrap();
        assert_ne!(
            a.payload_json,
            report(&base, &changed).unwrap().payload_json
        );
        assert_eq!(
            base.digest(),
            fmine_public::recovery::run_synthetic().unwrap().digest()
        );
        let mut unavailable = base;
        unavailable.quarters[0].outcome = Err(Refusal::new(
            fmine_evidence::RefusalCode::SourceCellUnusable208,
            "synthetic missing observation",
        ));
        let refused = report(&unavailable, &options).unwrap();
        assert_ne!(refused.exit_code, 0);
        assert_eq!(
            refused
                .payload_json
                .matches("\"status\":\"refused\",\"claim\":null")
                .count(),
            4
        );
    }
}
