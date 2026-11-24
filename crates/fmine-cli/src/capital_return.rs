//! Exact capital return from declared worksheets or authenticated registered filing transcriptions.
//! Determinism: Exact

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::definitions::capital_return::{self as roce, source, worksheet};
use fmine_metrics::guidance::Period;
use fmine_public::sources::{is_issuer_name, load_issuer_profile};
use fmine_public::transcribe::load_transcription_quantities_for_period_as_of;

use crate::{Envelope, NextAction, json_string as j, worksheet_input};

pub const USAGE: &str = "metrics roce --input <worksheet.toml> | --issuer <key> --document <registered.pdf> --series <key> --period YYYYFY|YYYYHn|YYYYQn --as-of YYYY-MM-DD";
pub const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schema":"fmine.capital-return.v1","worksheet_schema":"fmine.capital-return-worksheet.v1","modes":{"declared":"--input <worksheet.toml>; at most 1 MiB; exact source bytes SHA-256 bound; Assumed, Internal","filing":"--issuer <key> --document <registered.pdf> --series <key> --period YYYYFY|YYYYHn|YYYYQn --as-of YYYY-MM-DD; registered financial filing, SHA-256 verified bytes, FACT publication evidence, two agreeing readers and complete selected role population"},"#,
    r#""common_fields":["subject","ownership","perimeter","accounting_basis","currency","monetary_scale","definition","version","earnings_basis","tax_basis","capital_employed_basis","capital_basis","annualization","applicability"],"worksheet_top_fields":["schema","issuer","period"],"optional_fields":["ownership_share","annualization_factor","annualization_basis","exclusion_reason"],"units":"Explicit supported ISO currency and monetary_scale 1, 1000 or 1000000; no FX, scale or ownership conversion","tax_basis":"pre_tax|after_tax; earnings_basis names all included adjustments, none inferred","capital_basis":"opening|closing|opening_closing_mean|reported_average; exactly the selected operands, no unused balances or fallback","annualization":"none without optional annualization fields, or explicit with positive annualization_factor and named annualization_basis; raw period return always retained","applicability":"applicable or excluded with exclusion_reason; an excluded series never becomes a computed return","#,
    r#""worksheet_rows":{"earnings":["period","amount","rounding_step"],"opening":["as_of","amount","rounding_step"],"closing":["as_of","amount","rounding_step"],"average":["period","amount","rounding_step"]},"worksheet_values":"All fields quoted strings; amounts exact decimal or n/d, positive denominator; rounding_step zero means explicitly exact, positive means closed half-step rounding","filing_rows":"Quantity keys <series>_earnings, <series>_opening, <series>_closing, <series>_average; source unit USD/USD_k/USD_m etc.; source period matches selected period","filing_basis":"Semicolon-separated key=value common/optional fields plus definition_page, definition_table, definition_row, definition_col; opening/closing also require snapshot=YYYY-MM-DD. Every semantic field must agree across rows. Source printed precision is retained, not overridden","#,
    r#""domain":"Same issuer, subject, complete calendar-month reporting window, ownership, operating perimeter, accounting basis and monetary unit; opening is prior period close, closing is target period close. Full selected capital rounding enclosure strictly positive; signed earnings retained","output":"Exact numerator and capital enclosures, period and explicit annualized return percentages, definition identity, complete canonical source/intermediate/result Claims and anchors; NonAdditive","limits":"No automatic tax/interest/ownership adjustments, no continuous capital trajectory, no time-weighted average from two endpoints, no cross-definition ranking, no confidence-interval interpretation. Dash/n/a source tokens are unavailable under the existing transcription loader, never zero","example":"tests/metrics/capital_return.toml"}"#,
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Worksheet {
        input: PathBuf,
    },
    Filing {
        issuer: String,
        document: String,
        series: String,
        period: Period,
        as_of: String,
    },
}

fn period(text: &str) -> Result<Period, String> {
    if !text.is_ascii() || text.len() != 6 {
        return Err("metrics roce period requires YYYYFY, YYYYH1..H2 or YYYYQ1..Q4".into());
    }
    let year = text[..4]
        .parse::<u16>()
        .ok()
        .filter(|y| (1900..=9999).contains(y))
        .ok_or("metrics roce period requires a year between 1900 and 9999")?;
    let (first_month, last_month) = match &text[4..] {
        "FY" => (1, 12),
        "H1" => (1, 6),
        "H2" => (7, 12),
        "Q1" => (1, 3),
        "Q2" => (4, 6),
        "Q3" => (7, 9),
        "Q4" => (10, 12),
        _ => return Err("metrics roce period requires YYYYFY, YYYYH1..H2 or YYYYQ1..Q4".into()),
    };
    Ok(Period {
        year,
        first_month,
        last_month,
    })
}

/// Shared workspace/output-format options have already been removed by the main router.
pub fn parse(words: &[String]) -> Result<Command, String> {
    if words.first().map(String::as_str) != Some("metrics")
        || words.get(1).map(String::as_str) != Some("roce")
    {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    let mut i = 2;
    while i < words.len() {
        let flag = words[i].as_str();
        if !matches!(
            flag,
            "--input" | "--issuer" | "--document" | "--series" | "--period" | "--as-of"
        ) {
            return Err(format!("metrics roce: unsupported option {flag}; {USAGE}"));
        }
        let value = words
            .get(i + 1)
            .filter(|v| !v.trim().is_empty() && !v.starts_with("--"))
            .ok_or_else(|| format!("metrics roce: missing value for {flag}"))?;
        if fields.insert(flag, value.as_str()).is_some() {
            return Err(format!("metrics roce: duplicate {flag}"));
        }
        i += 2;
    }
    if let Some(input) = fields.get("--input") {
        if fields.len() != 1 {
            return Err("metrics roce --input cannot be mixed with filing selectors".into());
        }
        return Ok(Command::Worksheet {
            input: PathBuf::from(input),
        });
    }
    let required = |key| {
        fields
            .get(key)
            .copied()
            .ok_or_else(|| format!("metrics roce requires {key}; {USAGE}"))
    };
    let issuer = required("--issuer")?;
    if !is_issuer_name(issuer) {
        return Err("metrics roce --issuer requires a registered issuer key".into());
    }
    let series = required("--series")?;
    if !source::valid_series(series) {
        return Err(
            "metrics roce --series requires 1..128 lowercase letters, digits or underscores".into(),
        );
    }
    let as_of = required("--as-of")?;
    if !fmine_registry::sources::is_date(as_of) {
        return Err("metrics roce --as-of requires YYYY-MM-DD".into());
    }
    Ok(Command::Filing {
        issuer: issuer.into(),
        document: required("--document")?.into(),
        series: series.into(),
        period: period(required("--period")?)?,
        as_of: as_of.into(),
    })
}

fn bad(code: RefusalCode, detail: &str) -> Refusal {
    Refusal::new(code, format!("capital return: {detail}"))
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (receipt, provenance, mode) = match command {
            Command::Worksheet { input } => {
                let (text, anchor) = worksheet_input::read(
                    &root.join(input),
                    "capital-return worksheet",
                    worksheet::MAX_BYTES as u64,
                )?;
                let receipt = worksheet::evaluate(&text, anchor.clone())?;
                (
                    receipt,
                    worksheet_input::source_json(&anchor, text.len()),
                    "declared_worksheet",
                )
            }
            Command::Filing {
                issuer,
                document,
                series,
                period,
                as_of,
            } => {
                let profile = load_issuer_profile(root, issuer)?;
                let selected = profile
                    .documents
                    .iter()
                    .filter(|d| d.file == *document)
                    .collect::<Vec<_>>();
                let [document] = selected.as_slice() else {
                    return Err(bad(
                        RefusalCode::NoClaimInDomain201,
                        "exactly one explicitly selected registered document is required",
                    ));
                };
                if document.published_at > *as_of {
                    return Err(bad(
                        RefusalCode::AssayLookahead108,
                        "selected filing was published after the evidence cut",
                    ));
                }
                let period_label = fmine_metrics::mirror::transcriptions::period_label(*period);
                let loaded = load_transcription_quantities_for_period_as_of(
                    &profile,
                    root,
                    as_of,
                    &source::quantities(series),
                    Some(&document.file),
                    &period_label,
                )?
                .ok_or_else(|| {
                    bad(
                        RefusalCode::NotObservable202,
                        "selected issuer has no transcription registry",
                    )
                })?;
                if !loaded.agreement.is_consistent() || !loaded.agreement.single_reader.is_empty() {
                    return Err(bad(
                        RefusalCode::NotObservable202,
                        "complete independent reader agreement is required for every selected capital-return role",
                    ));
                }
                let inputs = source::bind(
                    issuer,
                    series,
                    *period,
                    document,
                    &loaded.agreement.accepted,
                )?;
                let receipt = roce::evaluate(&inputs)?;
                let provenance = format!(
                    "{{\"issuer\":{},\"document\":{},\"url\":{},\"sha256\":{},\"bytes\":{},\"published_at\":{},\"published_at_basis\":{},\"as_of\":{},\"series\":{},\"transcription_registry\":{}}}",
                    j(issuer),
                    j(&document.file),
                    j(&document.url),
                    j(&document.sha256),
                    document.bytes,
                    j(&document.published_at),
                    j(&document.published_at_basis),
                    j(as_of),
                    j(series),
                    j(&loaded.registry)
                );
                (receipt, provenance, "registered_filing")
            }
        };
        let payload = format!(
            "{{\"schema\":\"fmine.capital-return.v1\",\"mode\":{},\"source\":{provenance},\"receipt\":{}}}",
            j(mode),
            receipt.to_json()
        );
        let mut envelope = Envelope::ok("metrics roce", payload, Vec::new());
        envelope.no_claim = Some(roce::NO_CLAIM);
        envelope.rendered_markdown = Some(receipt.to_markdown());
        if mode == "declared_worksheet" {
            envelope.warnings.push("Declared earnings, capital and definitions are Assumed, Internal and uncalibrated.".into());
        }
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| {
        let mut envelope = Envelope::refused(
            "metrics roce",
            refusal,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect capital-return definitions, source roles and dated capital requirements",
            )],
        );
        envelope.no_claim = Some(roce::NO_CLAIM);
        envelope
    })
}
