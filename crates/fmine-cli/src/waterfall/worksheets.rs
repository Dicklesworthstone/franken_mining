//! DESIGN (BD-064/BD-333): exact copper cost bridges over two admitted declared worksheets.
//! The complete original receipts retain Assumed/Internal evidence and every excluded input.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::{Encoder, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::definitions::claims::{CostReceipt, CostValue, Treatment};
use fmine_metrics::definitions::{CostUnit, worksheet};
use fmine_metrics::guidance::Ownership;
use fmine_metrics::lmdi::waterfall::copper::{self, Metric, SalesUnit};
use fmine_metrics::lmdi::{BarPoint, BasisTag, PrintedQty};
use fmine_numerics::exact_feas::Rat;
use fmine_registry::metrics::{MetricFamily, MetricRegistry};
use fmine_units::UnitsError;

use crate::worksheet_input::{markdown_cell as text, read, source_json};
use crate::{Envelope, NextAction, json_string as js};

pub const USAGE: &str = "metrics waterfall --from-worksheet <file.toml> --to-worksheet <file.toml> --definition <registered-copper-cost-id>";
pub const NO_CLAIM: &str = "Exact descriptive copper accounting bridge conditional on two declared complete cost populations and one registered definition. Inputs remain Assumed, Internal and uncalibrated. No issuer-source authentication, causal attribution, calibrated uncertainty, disclosure eligibility or operating authorization is supplied.";
pub const ROBOT_DOCS_JSON: &str = r#"{"command":"metrics waterfall --from-worksheet <file.toml> --to-worksheet <file.toml> --definition <registered-copper-cost-id>","schema":"fmine.copper-waterfall.worksheets.v1","inputs":"Two existing fmine.cost-worksheet.v1 files with pounds_sold, and the exact workspace metric registry; regular UTF-8 files at most 4 MiB each","definition":"Explicit registered copper CashCost or AllInCost definition in USD/lb; both receipts bind the same definition tag","scope":"Identical issuer, subject, ownership, accounting basis and primary-metal allocation; chronologically increasing, nonoverlapping periods of the same number of months","arithmetic":"Signed included USD contributions and reciprocal copper pounds sold; symmetric additive split closes exactly; negative costs and deductions stay signed","population":"Every included item and treatment must match across periods; an absent component is not an inferred zero; excluded inputs remain visible in the complete original receipts","units":"All output effects and recomputed rates are USD/lb; no gold-ounce conversion and no use of produced copper","evidence":"Both full source-bound cost receipts, worksheet and registry hashes, result digest, every bar dependency, false authority flags; Assumed/Internal/uncalibrated","limits":"Worksheet decimals are exact declarations. Numeric enclosures are not source uncertainty or confidence intervals. No independent reported rate is available, so reported-rate gaps remain null. Registered workbook ingestion and physical LMDI attribution are outside this mode"}"#;

const MAX_BYTES: u64 = 4 * 1024 * 1024;
const COMPONENT_GROUPING: &str = "Each component is the exact sum of signed numerator_usd contributions with the same registered item and treatment in its original receipt. All underlying project, sales and cost Claims remain in that receipt. Excluded inputs stay visible there; no absent included component is filled with zero.";
type Result<T> = std::result::Result<T, Refusal>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub from: PathBuf,
    pub to: PathBuf,
    pub definition: String,
}

pub fn selected(args: &[String]) -> bool {
    args.iter()
        .any(|value| matches!(value.as_str(), "--from-worksheet" | "--to-worksheet"))
}

/// Common output and workspace flags are removed by the main router.
pub fn parse(args: &[String]) -> std::result::Result<Command, String> {
    if args.first().map(String::as_str) != Some("metrics")
        || args.get(1).map(String::as_str) != Some("waterfall")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut index = 2;
    while index < args.len() {
        let flag = args[index].as_str();
        if !matches!(flag, "--from-worksheet" | "--to-worksheet" | "--definition") {
            return Err(format!(
                "copper waterfall: unknown or mixed-mode option {flag}; {USAGE}"
            ));
        }
        let value = args
            .get(index + 1)
            .filter(|value| {
                !value.is_empty()
                    && value.trim() == value.as_str()
                    && !value.starts_with("--")
                    && !value.chars().any(char::is_control)
            })
            .ok_or_else(|| format!("copper waterfall: missing or invalid value for {flag}"))?;
        if fields.insert(flag, value).is_some() {
            return Err(format!("copper waterfall: duplicate {flag}"));
        }
        index += 2;
    }
    let get = |name| {
        fields
            .get(name)
            .map(|value| value.as_str())
            .ok_or_else(|| format!("copper waterfall requires {name}"))
    };
    let from = PathBuf::from(get("--from-worksheet")?);
    let to = PathBuf::from(get("--to-worksheet")?);
    if from == to {
        return Err("copper waterfall requires two distinct period worksheets".into());
    }
    Ok(Command {
        from,
        to,
        definition: get("--definition")?.into(),
    })
}

fn basis(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::BasisMismatch105, detail)
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "copper worksheet waterfall exact arithmetic capacity exceeded",
        )
    })
}

fn rat(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}

fn compatible(first: &CostReceipt, second: &CostReceipt) -> Result<()> {
    let (a, b) = (first.summary(), second.summary());
    if a.unit != CostUnit::CopperPoundSold
        || b.unit != CostUnit::CopperPoundSold
        || a.scope.issuer != b.scope.issuer
        || a.scope.subject != b.scope.subject
        || a.scope.ownership != b.scope.ownership
        || a.scope.basis != b.scope.basis
        || a.definition_tag != b.definition_tag
        || a.primary_metal_share != b.primary_metal_share
    {
        return Err(basis(
            "copper waterfall requires the same copper unit, issuer, subject, ownership, accounting definition and allocation",
        ));
    }
    let (a, b) = (a.scope.period, b.scope.period);
    if (a.year, a.last_month) >= (b.year, b.first_month)
        || a.last_month - a.first_month != b.last_month - b.first_month
    {
        return Err(basis(
            "copper waterfall requires increasing nonoverlapping periods of equal duration",
        ));
    }
    Ok(())
}

fn period(receipt: &CostReceipt, metric: Metric) -> Result<copper::Period> {
    let s = receipt.summary();
    let million = Rat::from_integer(1_000_000);
    let q = |value, at| PrintedQty {
        value,
        half_step: Rat::from_integer(0),
        at,
    };
    let sold = receipt
        .inputs()
        .iter()
        .find(|input| matches!(input.value.value, CostValue::SoldPounds(_)))
        .ok_or_else(|| basis("admitted copper worksheet has no sold-pound Claim"))?;
    let mut grouped = BTreeMap::new();
    for line in s
        .contributions
        .iter()
        .filter(|line| line.treatment != Treatment::Excluded)
    {
        let total = grouped
            .entry((line.item.as_str(), line.treatment.as_str()))
            .or_insert(Rat::from_integer(0));
        *total = exact(total.checked_add(line.numerator_usd))?;
    }
    let lines = grouped
        .into_iter()
        .map(|((item, treatment), total)| {
            Ok((
                format!("{item}:{treatment}"),
                q(
                    exact(total.checked_div(million))?,
                    format!(
                        "claim:{}:sum_contributions[item={},treatment={}]",
                        receipt.claim().claim_id,
                        js(item),
                        js(treatment),
                    ),
                ),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(copper::Period {
        metric,
        definition: s.definition_tag.clone(),
        cost: q(
            exact(s.numerator_usd.checked_div(million))?,
            format!("claim:{}:numerator_usd", receipt.claim().claim_id),
        ),
        lines,
        sold: q(
            s.quantity_sold,
            format!("claim:{}:pounds_sold", sold.claim_id),
        ),
        sold_unit: SalesUnit::Pounds,
        reported_rate: None,
        basis: match s.scope.ownership {
            Ownership::Full => BasisTag::Consolidated100,
            Ownership::Attributable { share } => BasisTag::Attributable { share },
        },
    })
}

fn metric(registry: &MetricRegistry, definition: &str) -> Result<Metric> {
    let definition = registry
        .definition(definition)
        .ok_or_else(|| basis("copper waterfall definition is unregistered"))?;
    match (definition.family, definition.unit.as_str()) {
        (MetricFamily::CashCost, "USD/lb") => Ok(Metric::C1),
        (MetricFamily::AllInCost, "USD/lb") => Ok(Metric::Aisc),
        _ => Err(basis(
            "copper waterfall requires a registered copper CashCost or AllInCost definition in USD/lb",
        )),
    }
}

fn bridge_json(bridge: &copper::Bridge) -> Result<String> {
    let bars = bridge
        .bars
        .iter()
        .map(|bar| {
            let BarPoint::Exact(point) = bar.point else {
                return Err(basis("copper additive bar lost its exact rational point"));
            };
            let inputs = bar
                .inputs
                .iter()
                .map(|value| js(value))
                .collect::<Vec<_>>()
                .join(",");
            Ok(format!(
                "{{\"id\":{},\"value\":{},\"unit\":\"usd_per_lb\",\"rule\":{},\"numeric_enclosure\":[{},{}],\"sign_not_determined\":{},\"inputs\":[{inputs}]}}",
                js(&bar.id),
                js(&rat(point)),
                js(bar.rule),
                bar.enclosure[0],
                bar.enclosure[1],
                bar.sign_not_determined,
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(",");
    let inputs = bridge
        .rates
        .inputs
        .iter()
        .map(|value| js(value))
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"rate_0\":{},\"rate_1\":{},\"total_delta\":{},\"unit\":\"usd_per_lb\",\"rule\":\"copper.waterfall.rate_reconciliation\",\"inputs\":[{inputs}],\"reported_rate_0\":null,\"reported_rate_1\":null,\"reported_rate_gap_0\":null,\"reported_rate_gap_1\":null,\"reported_rate_gap\":null,\"bars\":[{bars}]}}",
        js(&rat(bridge.rates.rate_0)),
        js(&rat(bridge.rates.rate_1)),
        js(&rat(bridge.rates.total_delta)),
    ))
}

fn markdown(
    first: &CostReceipt,
    second: &CostReceipt,
    bridge: &copper::Bridge,
    digest: &str,
) -> Result<String> {
    let s = first.summary();
    let mut out = format!(
        "# Declared copper cost waterfall\n\n{} / {} under {}.\n\nRecomputed rate: {} to {} USD/lb; change {} USD/lb.\n\nDigest: {}.\n\nBoth source worksheets remain Assumed, Internal and uncalibrated. Decimal inputs are exact declarations; numerical enclosures are not observation uncertainty or confidence intervals. No independently reported rate was supplied, so no reported-rate gap is inferred.\n\n{COMPONENT_GROUPING}\n\n| Component | Change USD/lb | Numeric enclosure USD/lb | Sign |\n|---|---:|---|---|\n",
        text(&s.scope.issuer),
        text(&s.scope.subject),
        text(&s.definition),
        rat(bridge.rates.rate_0),
        rat(bridge.rates.rate_1),
        rat(bridge.rates.total_delta),
        text(digest),
    );
    for bar in &bridge.bars {
        let BarPoint::Exact(point) = bar.point else {
            return Err(basis("copper additive bar lost its exact rational point"));
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} to {} | {} |",
            text(&bar.id),
            rat(point),
            bar.enclosure[0],
            bar.enclosure[1],
            if bar.sign_not_determined {
                "not determined"
            } else {
                "determined"
            },
        );
    }
    out.push_str("\n## Original period receipt\n\n");
    out.push_str(&first.to_markdown());
    out.push_str("\n## Later period receipt\n\n");
    out.push_str(&second.to_markdown());
    let _ = writeln!(out, "\n{NO_CLAIM}");
    Ok(out)
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope> {
        let (metrics_text, metrics_source) = read(
            &root.join("registries/metrics.toml"),
            "copper waterfall metric registry",
            MAX_BYTES,
        )?;
        let registry = MetricRegistry::parse(&metrics_text)
            .map_err(|error| basis(&format!("copper waterfall registry: {error}")))?;
        let selected_metric = metric(&registry, &command.definition)?;
        let (first_text, first_source) = read(
            &root.join(&command.from),
            "first copper worksheet",
            MAX_BYTES,
        )?;
        let (second_text, second_source) =
            read(&root.join(&command.to), "later copper worksheet", MAX_BYTES)?;
        let first = worksheet::evaluate(
            &first_text,
            first_source.clone(),
            &metrics_text,
            metrics_source.clone(),
            &command.definition,
        )?;
        let second = worksheet::evaluate(
            &second_text,
            second_source.clone(),
            &metrics_text,
            metrics_source.clone(),
            &command.definition,
        )?;
        compatible(&first, &second)?;
        let bridge = copper::decompose(
            &period(&first, selected_metric)?,
            &period(&second, selected_metric)?,
        )?;
        let rendered_bridge = bridge_json(&bridge)?;
        let mut frame = Encoder::new_frame(*b"CWFL", 1);
        frame.put_str(copper::RULES_VERSION);
        frame.put_str(COMPONENT_GROUPING);
        frame.put_str(first.digest());
        frame.put_str(second.digest());
        frame.put_str(&rendered_bridge);
        let digest = blake3_hash(&frame.finish()).to_hex();
        let payload = format!(
            "{{\"schema\":\"fmine.copper-waterfall.worksheets.v1\",\"mode\":\"declared_copper_cost_worksheets\",\"rules_version\":{},\"component_grouping\":{},\"unit\":\"usd_per_lb\",\"waterfall_digest\":{},\"sources\":[{},{}],\"registry\":{},\"from_receipt\":{},\"to_receipt\":{},\"bridge\":{rendered_bridge}}}",
            js(copper::RULES_VERSION),
            js(COMPONENT_GROUPING),
            js(&digest),
            source_json(&first_source, first_text.len()),
            source_json(&second_source, second_text.len()),
            source_json(&metrics_source, metrics_text.len()),
            first.to_json(),
            second.to_json(),
        );
        let mut envelope = Envelope::ok(
            "metrics waterfall",
            payload,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the copper worksheet waterfall contract",
            )],
        );
        envelope.no_claim = Some(NO_CLAIM);
        envelope.warnings.push(
            "Source costs, completeness, classification and timing are declared, not independently authenticated."
                .into(),
        );
        envelope.rendered_markdown = Some(markdown(&first, &second, &bridge, &digest)?);
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "metrics waterfall",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the copper worksheet waterfall contract",
            )],
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tail: &[&str]) -> Vec<String> {
        ["metrics", "waterfall"]
            .into_iter()
            .chain(tail.iter().copied())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn parses_two_declared_worksheets_without_public_mode_options() {
        let input = args(&[
            "--to-worksheet",
            "later period.toml",
            "--from-worksheet",
            "first.toml",
            "--definition",
            "c1.copper@barrick-q2-2026",
        ]);
        assert!(selected(&input));
        assert_eq!(
            parse(&input).unwrap(),
            Command {
                from: "first.toml".into(),
                to: "later period.toml".into(),
                definition: "c1.copper@barrick-q2-2026".into(),
            }
        );
        assert!(!selected(&args(&[
            "--issuer", "barrick", "--from", "2030Q4", "--to", "2031Q1",
        ])));
    }

    #[test]
    fn missing_duplicate_same_file_and_mixed_mode_arguments_are_usage_errors() {
        for tail in [
            vec!["--from-worksheet", "a.toml"],
            vec!["--from-worksheet"],
            vec![
                "--from-worksheet",
                "a.toml",
                "--to-worksheet",
                "a.toml",
                "--definition",
                "c1.copper@barrick-q2-2026",
            ],
            vec![
                "--from-worksheet",
                "a.toml",
                "--to-worksheet",
                "b.toml",
                "--definition",
                "c1.copper@barrick-q2-2026",
                "--definition",
                "other",
            ],
            vec![
                "--from-worksheet",
                "a.toml",
                "--to-worksheet",
                "b.toml",
                "--definition",
                "c1.copper@barrick-q2-2026",
                "--issuer",
                "barrick",
            ],
            vec![
                "--from-worksheet",
                "a.toml",
                "--to-worksheet",
                "b.toml",
                "--definition",
                "c1.copper@barrick-q2-2026",
                "--metric",
                "tcc",
            ],
            vec![
                "--from-worksheet",
                "a.toml",
                "--to-worksheet",
                " b.toml ",
                "--definition",
                "c1.copper@barrick-q2-2026",
            ],
        ] {
            assert!(parse(&args(&tail)).is_err(), "accepted {tail:?}");
        }
    }
}
