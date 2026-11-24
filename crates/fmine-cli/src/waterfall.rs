//! DESIGN (BD-333): explicitly selected, source-bound quarterly public AISC bridges.

use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::lmdi::WATERFALL_NO_CLAIM;
use fmine_public::key::{Period, PeriodKind, parse_period_code};
use fmine_public::sources::{SourceKind, is_issuer_name, load_issuer_profile};
use fmine_public::waterfall::{BarPoint, WaterfallReport, pair_sites_between};
use fmine_public::{Lane, assess, load_registered_sources};

use crate::guidance::{number, text};
use crate::{Envelope, NextAction, json_string as js};

pub const USAGE: &str =
    "metrics waterfall --issuer <issuer> --from YYYYQn --to YYYYQn [--site <subject>]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub issuer: String,
    pub from: Period,
    pub to: Period,
    pub site: Option<String>,
}

fn quarter(value: &str) -> Result<Period, String> {
    parse_period_code(value)
        .filter(|p| p.year > 0 && matches!(p.kind, PeriodKind::Quarter(1..=4)))
        .filter(|p| p.code() == value)
        .ok_or_else(|| {
            format!("metrics waterfall requires calendar quarters YYYYQ1 through YYYYQ4: {value}")
        })
}

/// Common workspace and output options have already been removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("metrics")
        || args.get(1).map(String::as_str) != Some("waterfall")
    {
        return Err(USAGE.into());
    }
    let (mut issuer, mut from, mut to, mut site) = (None, None, None, None);
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        let slot = match flag {
            "--issuer" => &mut issuer,
            "--from" => &mut from,
            "--to" => &mut to,
            "--site" => &mut site,
            _ => return Err(format!("metrics waterfall: unknown option {flag}; {USAGE}")),
        };
        let value = args
            .get(i + 1)
            .filter(|s| !s.trim().is_empty() && !s.starts_with("--"))
            .ok_or_else(|| format!("metrics waterfall: missing value for {flag}"))?;
        if value.trim() != value || value.chars().any(char::is_control) {
            return Err(format!("metrics waterfall: invalid value for {flag}"));
        }
        if slot.replace(value.clone()).is_some() {
            return Err(format!("metrics waterfall: duplicate {flag}"));
        }
        i += 2;
    }
    let issuer = issuer.ok_or("metrics waterfall requires --issuer")?;
    if !is_issuer_name(&issuer) {
        return Err("metrics waterfall requires a registered issuer name".into());
    }
    let from = quarter(&from.ok_or("metrics waterfall requires --from")?)?;
    let to = quarter(&to.ok_or("metrics waterfall requires --to")?)?;
    if from >= to {
        return Err("metrics waterfall requires --from earlier than --to".into());
    }
    Ok(Command {
        issuer,
        from,
        to,
        site,
    })
}

fn markdown(command: &Command, report: &WaterfallReport) -> String {
    let mut out = format!(
        "# Quarterly AISC cost bridge\n\n{}: {} to {}.\n\nOnly each selected quarter's original registered release supplies its bridge inputs. Earlier original Mine Stats releases from each selected year support the existing cumulative identity checks. Later comparative columns never fill a missing original. Amounts below are USD per sold gold ounce. Rounding enclosures are not confidence intervals.\n\nDigest: {}.\n\n",
        text(&command.issuer),
        command.from.code(),
        command.to.code(),
        report.digest()
    );
    for pair in &report.pairs {
        let _ = writeln!(out, "## {}\n", text(&pair.subject));
        match &pair.outcome {
            Err(refusal) => {
                let _ = writeln!(
                    out,
                    "Refused — {}: {}\n\nRequired evidence: {}\n",
                    refusal.code,
                    text(&refusal.what),
                    text(refusal.fix)
                );
            }
            Ok(bridge) => {
                let gap = bridge.gap();
                let _ = writeln!(
                    out,
                    "Method: {}. Recomputed rate: {} to {}; change {}. Printed rate: {} to {}. The change in the printed-rate gap is {} and is kept separate from the bars.\n\n| Component | Change USD/oz | Rounding enclosure USD/oz | Sign |\n|---|---:|---|---|",
                    pair.kind(),
                    number(gap.aisc_0),
                    number(gap.aisc_1),
                    number(gap.total_delta),
                    number(gap.printed_aisc_0),
                    number(gap.printed_aisc_1),
                    number(gap.printed_rate_gap)
                );
                for bar in bridge.bars() {
                    let point = match bar.point {
                        BarPoint::Exact(value) => number(value),
                        BarPoint::Approx(value) => format!("approximately {value}"),
                    };
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} to {} | {} |",
                        text(&bar.id),
                        point,
                        bar.enclosure[0],
                        bar.enclosure[1],
                        if bar.sign_not_determined {
                            "not determined"
                        } else {
                            "determined"
                        }
                    );
                }
                out.push('\n');
            }
        }
        for cell in &pair.inputs {
            let _ = writeln!(
                out,
                "- Source Claim {}: {}; SHA-256 {}.",
                cell.claim.claim_id,
                text(&cell.locator),
                cell.key.reporting_vintage.file_sha256
            );
        }
        for (locator, digest) in &pair.context {
            let _ = writeln!(
                out,
                "- Source context: {}; SHA-256 {}.",
                text(locator),
                digest
            );
        }
        out.push('\n');
    }
    let _ = writeln!(out, "{WATERFALL_NO_CLAIM}");
    out
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let mut profile = load_issuer_profile(root, &command.issuer)?;
        let from = command.from.code();
        let to = command.to.code();
        // Retain preceding originals needed to check each selected release's cumulative identities.
        // Later comparative columns cannot supply those originals or either bridge endpoint.
        profile.sources.retain(|source| {
            source.quarter == from
                || source.quarter == to
                || (source.kind == SourceKind::MineStats
                    && parse_period_code(&source.quarter).is_some_and(|period| {
                        [command.from, command.to]
                            .into_iter()
                            .any(|endpoint| period.year == endpoint.year && period <= endpoint)
                    }))
        });
        let sources = load_registered_sources(&profile, root)?;
        let lane = if profile.issuer == fmine_public::synthetic::SYNTHETIC_ISSUER {
            Lane::Synthetic
        } else {
            Lane::Public
        };
        let snapshot = assess(&profile, lane, sources);
        let mut report = pair_sites_between(
            &profile,
            &snapshot.sources,
            &snapshot.mine_stats,
            &snapshot.public_claims,
            &snapshot.instances,
            &snapshot.mine_ledger,
            (command.from, command.to),
        )?;
        if let Some(site) = &command.site {
            report.pairs.retain(|pair| &pair.subject == site);
            if report.pairs.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    format!("no original source assertions for {site} at {from} or {to}"),
                ));
            }
        }
        let payload = format!(
            "{{\"schema\":\"fmine.selected-waterfall.v1\",\"issuer\":{},\"lane\":{},\"from\":{},\"to\":{},\"site\":{},\"waterfall_digest\":{},\"waterfall\":{}}}",
            js(&command.issuer),
            js(if lane == Lane::Synthetic {
                "synthetic"
            } else {
                "public"
            }),
            js(&from),
            js(&to),
            command.site.as_deref().map_or_else(|| "null".into(), js),
            js(&report.digest().to_string()),
            report.to_json()
        );
        let mut envelope = Envelope::ok("metrics waterfall", payload, vec![]);
        envelope.no_claim = Some(WATERFALL_NO_CLAIM);
        envelope.rendered_markdown = Some(markdown(command, &report));
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        Envelope::refused(
            "metrics waterfall",
            refusal,
            vec![NextAction::new(
                "fmine doctor",
                "inspect the registered original workbook sources",
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
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn selected_quarters_route_with_common_options_and_literal_subject() {
        let input = args(&[
            "--to",
            "2031Q1",
            "--issuer",
            "barrick",
            "--from",
            "2030Q4",
            "--site",
            "North Mara",
            "--format",
            "markdown",
        ]);
        let (command, format, _) = crate::parse_args(&input, Path::new(".")).unwrap();
        let crate::CliCommand::MetricsWaterfall(selected) = command else {
            panic!("wrong CLI route");
        };
        assert_eq!(format, crate::OutputFormat::Markdown);
        assert_eq!(selected.from.code(), "2030Q4");
        assert_eq!(selected.to.code(), "2031Q1");
        assert_eq!(selected.site.as_deref(), Some("North Mara"));
    }

    #[test]
    fn incomplete_reversed_nonquarter_and_duplicate_selections_are_usage_errors() {
        for tail in [
            vec!["--issuer", "barrick", "--from", "2030Q1"],
            vec!["--issuer", "barrick", "--from", "2031Q1", "--to", "2030Q4"],
            vec!["--issuer", "barrick", "--from", "2030Q1", "--to", "2030Q1"],
            vec!["--issuer", "barrick", "--from", "2030H1", "--to", "2031Q1"],
            vec!["--issuer", "barrick", "--from", "0000Q1", "--to", "2031Q1"],
            vec![
                "--issuer", "barrick", "--from", "2030Q1", "--to", "2031Q1", "--to", "2032Q1",
            ],
            vec![
                "--issuer",
                "../barrick",
                "--from",
                "2030Q1",
                "--to",
                "2031Q1",
            ],
            vec![
                "--issuer", "barrick", "--from", "2030Q1", "--to", "2031Q1", "--site",
            ],
        ] {
            assert!(parse(&args(&tail)).is_err(), "accepted {tail:?}");
        }
    }
}
