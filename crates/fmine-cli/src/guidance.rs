//! Human-readable guidance requirements and the exact historical comparison behind them.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use fmine_claim::Claim;
use fmine_core::Digest32;
use fmine_metrics::capacity::{CapacityVerdict, PhysicalLimit};
use fmine_metrics::guidance::{
    EnvelopeTier, HistoryVerdict, Ownership, Period, Quantity, QuantityKind,
};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_public::guidance::capacity::{TechnicalAssessment, TechnicalScenarioKind};
use fmine_public::guidance::{GuidanceCompletionReport, NO_CLAIM};

pub(crate) fn text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(['\n', '\r'], " ")
}

/// Prefer an exact terminating decimal; retain a nonterminating rational and label its display approximation.
pub(crate) fn number(value: Rat) -> String {
    let (num, den) = (value.num(), value.den());
    let mut scale = 1_i128;
    for places in 0..=6 {
        if scale % den == 0
            && let Some(scaled) = num.checked_mul(scale / den)
        {
            if places == 0 {
                return scaled.to_string();
            }
            let magnitude = scaled.unsigned_abs();
            let base = scale.unsigned_abs();
            let fraction = format!("{:0places$}", magnitude % base);
            return format!(
                "{}{}.{}",
                if scaled < 0 { "−" } else { "" },
                magnitude / base,
                fraction.trim_end_matches('0')
            );
        }
        scale *= 10;
    }
    format!("{num}/{den} (≈ {:.2})", value.to_f64())
}

pub(crate) fn interval(value: Interval) -> String {
    if value.lo == value.hi {
        number(value.lo)
    } else {
        format!("{} to {}", number(value.lo), number(value.hi))
    }
}

fn period(value: Period) -> String {
    format!(
        "{} months {}–{}",
        value.year, value.first_month, value.last_month
    )
}

fn possibility(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "Indeterminate",
    }
}

fn quantity_unit(kind: QuantityKind) -> &'static str {
    match kind {
        QuantityKind::ProductionKoz | QuantityKind::SoldKoz => "koz",
        QuantityKind::ProductionKt => "kt",
        QuantityKind::SoldMillionLb => "million lb",
        QuantityKind::AiscUsdPerSoldOz => "USD per sold oz",
        QuantityKind::AiscUsdPerSoldLb | QuantityKind::C1UsdPerSoldLb => "USD per sold lb",
        QuantityKind::AiscSpendUsdMillion | QuantityKind::C1SpendUsdMillion => "USD million",
    }
}

fn row(out: &mut String, name: &str, claim: &Claim<Quantity>, unit: &str) {
    let _ = writeln!(
        out,
        "| {name} | {} {unit} | {} | `{}` |",
        interval(claim.value.interval),
        text(&format!("{:?}", claim.evidence)),
        claim.claim_id
    );
}

fn physical_row(out: &mut String, name: &str, claim: &Claim<PhysicalLimit>, unit: &str) {
    let _ = writeln!(
        out,
        "| {name} | {} {unit} | {} | `{}` |",
        interval(claim.value.interval),
        text(&format!("{:?}", claim.evidence)),
        claim.claim_id
    );
}

fn technical(out: &mut String, assessment: &TechnicalAssessment) {
    out.push_str("## Automatic technical-report scenario\n\n");
    let TechnicalAssessment::Compared(comparison) = assessment else {
        if let TechnicalAssessment::Indeterminate { reason } = assessment {
            let _ = writeln!(
                out,
                "**T3 — technical comparison indeterminate.** {}: {}\n",
                reason.code,
                text(&reason.what)
            );
        }
        out.push_str("The arithmetic requirements and any historical-window comparison remain available. This automatic selection supplies no production ceiling; an explicitly selected document is assessed separately.\n\n");
        return;
    };
    let scenario = match comparison.kind {
        TechnicalScenarioKind::DeclaredUpperBound => "declared upper bounds",
        TechnicalScenarioKind::Plan => "stated plan scenario",
        TechnicalScenarioKind::Nameplate => "stated nameplate scenario",
    };
    let verdict = match comparison.result.verdict {
        CapacityVerdict::ExceedsStatedProductionCeiling => {
            "The remaining production requirement exceeds the result of this stated scenario."
        }
        CapacityVerdict::NotExcluded => {
            "The stated scenario does not exclude the remaining production requirement. This does not establish feasibility."
        }
        CapacityVerdict::AlreadySatisfied => {
            "The production requirement is already satisfied across the supplied bounds."
        }
    };
    let copper = comparison.result.maximum_production.value.kind == QuantityKind::ProductionKt;
    let grade_unit = if copper {
        "copper mass fraction of dry feed"
    } else {
        "g per dry tonne"
    };
    let _ = writeln!(
        out,
        "**T2 — {scenario}.** {verdict}\n\nSource: {}; published {}; date basis: {}.\n\n\
         This comparison concerns production only. It does not supply a joint production/AISC witness or establish mine capacity. \
         Plan and nameplate values remain conditional scenarios. No annual rate is prorated and no grade average becomes an upper bound.\n\n\
         | Quantity | Exact bounds | Evidence | Claim |\n|---|---|---|---|",
        text(&comparison.document.file),
        text(&comparison.document.published_at),
        text(&comparison.document.published_at_basis)
    );
    for (name, claim, unit) in [
        (
            "Remaining dry feed, 100% mine basis",
            &comparison.limits.feed,
            "Mt",
        ),
        ("Feed head grade", &comparison.limits.grade, grade_unit),
        ("Recovery", &comparison.limits.recovery, "fraction"),
    ] {
        physical_row(out, name, claim, unit);
    }
    row(
        out,
        "Maximum production under supplied limits",
        &comparison.result.maximum_production,
        if copper {
            "kt, on the guidance ownership basis"
        } else {
            "koz, on the guidance ownership basis"
        },
    );
    physical_row(
        out,
        "Optimistic required head grade",
        &comparison.result.optimistic_required_grade,
        grade_unit,
    );
    out.push_str("\nThe required grade uses the highest supplied dry-feed and recovery bounds. A signed requirement is retained; the separate AISC frontier and historical tests still apply.\n\n\
                  | Page citation | Printed token | Unit | Basis | Period |\n|---|---|---|---|---|\n");
    for source in &comparison.sources {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            text(&source.locator.to_string()),
            text(&source.token),
            text(&source.unit),
            text(&source.basis),
            text(&source.period)
        );
    }
    out.push('\n');
}

pub(crate) fn render(report: &GuidanceCompletionReport, selection: &str) -> String {
    render_inner(report, selection, None)
}

pub(crate) fn render_with_capacity(
    report: &GuidanceCompletionReport,
    selection: &str,
    capacity_markdown: &str,
) -> String {
    render_inner(report, selection, Some(capacity_markdown))
}

fn render_inner(
    report: &GuidanceCompletionReport,
    selection: &str,
    capacity_markdown: Option<&str>,
) -> String {
    let no_claim = if capacity_markdown.is_some() {
        "Arithmetic and finite historical-window comparisons under a declared sales assumption. The separately requested production-ceiling comparison follows below. No forecast, probability of meeting guidance, joint production-cost feasibility or regulatory claim."
    } else {
        NO_CLAIM
    };
    let inputs = &report.inputs;
    let production_unit = quantity_unit(inputs.production_guidance.value.kind);
    let sold_unit = quantity_unit(inputs.remaining_sold.value.kind);
    let rate_unit = quantity_unit(inputs.aisc_guidance.value.kind);
    let cost = if inputs.aisc_guidance.value.kind == QuantityKind::C1UsdPerSoldLb {
        "C1"
    } else {
        "AISC"
    };
    let sold_metal = if inputs.remaining_sold.value.kind == QuantityKind::SoldMillionLb {
        "sold copper in millions of pounds"
    } else {
        "sold ounces"
    };
    let tier = match report.history.tier {
        EnvelopeTier::T1 => "T1 — finite historical windows",
        EnvelopeTier::T3 => "T3 — historical joint comparison unavailable",
    };
    let verdict = match report.history.verdict {
        HistoryVerdict::WithinEnvelope => {
            "A supplied historical window has an exact joint witness."
        }
        HistoryVerdict::ExceedsEnvelope => {
            "No supplied historical window satisfies the joint requirements under the stated sales scenario."
        }
        HistoryVerdict::Indeterminate => {
            "The supplied history does not establish a joint comparison; inspect the exclusions and indeterminate checks below."
        }
    };
    let ownership = match inputs.production_guidance.value.ownership {
        Ownership::Full => "100% production basis".to_string(),
        Ownership::Attributable { share } => {
            format!("Attributable production basis, share {}", number(share))
        }
    };
    let mut out = format!(
        "# Guidance completion: {}\n\n{}\n\n{}\n\n**{tier}.** {verdict}\n\n{}; remaining support: {}.\n\n\
         **Sales assumption:** repeat the preceding year's FY-minus-YTD {sold_metal} for the remaining months. \
         This assumption remains separate from production. Every historical comparison is conditional on that sales interval.\n\n\
         Report BLAKE3: `{}`. No forecast registration or ledger certificate is created.\n\n\
         ## Requirements\n\n| Quantity | Exact bounds | Evidence | Claim |\n|---|---|---|---|\n",
        text(&inputs.production_guidance.value.subject),
        text(selection),
        text(no_claim),
        ownership,
        period(inputs.remaining_sold.value.period),
        report.digest.to_hex()
    );
    for (name, claim) in [
        (
            "Remaining production".to_string(),
            &report.completion.remaining_production,
        ),
        (
            "Assumed remaining sales".to_string(),
            &inputs.remaining_sold,
        ),
        (
            format!("YTD {cost} spending"),
            &report.completion.ytd_aisc_spend,
        ),
        (
            format!("Required remaining {cost} spending"),
            &report.completion.remaining_aisc_spend,
        ),
        (
            format!("Required remaining {cost}"),
            &report.completion.remaining_aisc_rate,
        ),
    ] {
        row(&mut out, &name, claim, quantity_unit(claim.value.kind));
    }
    out.push_str("\nClosed bounds retain printed rounding uncertainty. Approximate decimal displays are marked ≈; the accompanying fractions remain exact. Signed net costs and requirements are retained, including rounding bounds that cross zero; sales remain strictly positive. The Evidence column preserves the weakest input evidence; it does not mean the issuer asserted the computed requirement or the replay assumption.\n\n");
    technical(&mut out, &report.technical);
    out.push_str("## Historical joint comparison\n\nA marginal production success and a marginal cost success do not establish joint feasibility. Only a checked joint witness does. Exclusion is limited to the supplied finite historical windows; it is not a mine-capacity conclusion or a forecast.\n\n");
    if report.history.checks.is_empty() {
        out.push_str("No complete admissible historical window was supplied.\n\n");
    } else {
        out.push_str("| Historical window | Same season | Production possible | Cost possible | Joint result |\n|---|---|---|---|---|\n");
        for check in &report.history.checks {
            let joint = match &check.witness {
                Some(witness) => format!("Checked witness `{}`", witness.claim_id),
                None if check.indeterminate.is_empty() => "Excluded within this window".into(),
                None => format!("Indeterminate: {}", text(&check.indeterminate.join("; "))),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {joint} |",
                text(&check.id),
                if check.same_season { "Yes" } else { "No" },
                possibility(check.production_possible),
                possibility(check.cost_possible)
            );
        }
        out.push('\n');
        for check in &report.history.checks {
            if let Some(witness) = &check.witness {
                let value = &witness.value;
                let _ = writeln!(
                    out,
                    "### Checked replay: {}\n\nAnnual production: {} {production_unit}; annual {cost}: {} {rate_unit}.\n\n| Original support | Hypothetical replay | Production ({production_unit}) | Sales ({sold_unit}) | {cost} ({rate_unit}) |\n|---|---|---|---|---|",
                    text(&check.id),
                    number(value.completion.annual_production),
                    number(value.completion.annual_aisc)
                );
                for quarter in &value.quarters {
                    let _ = writeln!(
                        out,
                        "| {} | {} | {} | {} | {} |",
                        period(quarter.source_period),
                        period(quarter.replay_period),
                        number(quarter.production),
                        number(quarter.sold),
                        number(quarter.aisc)
                    );
                }
                out.push('\n');
            }
        }
    }
    if !report.history_exclusions.is_empty() {
        out.push_str("### Excluded source windows\n\n| Window | Code | Missing or incompatible evidence |\n|---|---|---|\n");
        for item in &report.history_exclusions {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                text(&item.id),
                text(&item.code),
                text(&item.reason)
            );
        }
        out.push('\n');
    }
    out.push_str("## Guidance and actuals\n\n| Quantity | Exact bounds | Evidence | Claim |\n|---|---|---|---|\n");
    for (name, claim) in [
        (
            "Annual production guidance".to_string(),
            &inputs.production_guidance,
        ),
        (format!("Annual {cost} guidance"), &inputs.aisc_guidance),
        ("YTD production".to_string(), &inputs.ytd_production),
        ("YTD sales".to_string(), &inputs.ytd_sold),
        (format!("YTD {cost}"), &inputs.ytd_aisc),
    ] {
        row(&mut out, &name, claim, quantity_unit(claim.value.kind));
    }
    let mut anchors = BTreeSet::new();
    for claim in [
        &inputs.production_guidance,
        &inputs.aisc_guidance,
        &inputs.ytd_production,
        &inputs.ytd_sold,
        &inputs.ytd_aisc,
        &inputs.remaining_sold,
    ] {
        anchors.extend(claim.anchors.iter());
    }
    for window in &report.history_sources {
        for source in &window.source_claims {
            anchors.extend(source.claim.anchors.iter());
        }
    }
    if let TechnicalAssessment::Compared(comparison) = &report.technical {
        for claim in [
            &comparison.limits.feed,
            &comparison.limits.grade,
            &comparison.limits.recovery,
        ] {
            anchors.extend(claim.anchors.iter());
        }
    }
    out.push_str("\n## Source citations\n\n| Source | Locator | SHA-256 |\n|---|---|---|\n");
    for anchor in anchors {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` |",
            text(&anchor.artifact_path),
            text(&anchor.locator),
            Digest32(anchor.artifact_sha256).to_hex()
        );
    }
    out.push_str("\nAll four INV-002 authority flags remain false. Historical seasonality is context only. Technical comparisons are conditional on their cited scenarios and do not establish a probability of meeting guidance.\n");
    if let Some(capacity) = capacity_markdown {
        out.push_str("\nThe explicitly selected physical limits below supply a separate production-only exclusion test. They do not replace the automatic scenario or historical joint comparison.\n\n");
        out.push_str(capacity);
    }
    out
}

include!("../../xtask/testdata/guidance_controls.rs");
