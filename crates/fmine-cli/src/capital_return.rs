//! Read-only delivery of the definition-bound capital return engine (BD-061).
//! Determinism: Exact

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::definitions::capital_return::{self, CapitalReturnReceipt, source, worksheet};
use fmine_metrics::guidance::Period;
use fmine_public::sources::{is_issuer_name, load_issuer_profile};
use fmine_public::transcribe::load_transcription_quantities_as_of;
use fmine_registry::sources::is_date;

use crate::{Envelope, NextAction, json_string as js, worksheet_input};

const VERB: &str = "metrics capital-return";
const USAGE: &str = "metrics capital-return --input <worksheet.toml> | --issuer <issuer> --document <registered-file> --series <key> --period <YYYYFY|YYYYHn|YYYYQn> --as-of <YYYY-MM-DD>";
pub const ROBOT_DOCS_JSON: &str = r#"{"worksheet":"metrics capital-return --input tests/metrics/capital_return.toml","worksheet_schema":"fmine.capital-return-worksheet.v1","filing":"metrics capital-return --issuer <issuer> --document <registered-file> --series <key> --period <YYYYFY|YYYYHn|YYYYQn> --as-of <YYYY-MM-DD>","roles":"Agreed rows for the selected series use _earnings and the required _opening, _closing, or _average keys. Each retained locator needs at least two agreeing reader roles. Period, currency, scale, ownership, accounting and definition fields must agree, and opening/closing dates must match the respective endpoints.","output":"Original Claims and exact point returns with source-rounding enclosures and the declared period and annualized returns. No inferred annualization or accounting adjustment."}"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Worksheet(PathBuf),
    Filing {
        issuer: String,
        document: String,
        series: String,
        period: Period,
        as_of: String,
    },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    if !matches!(args, [group, verb, ..] if group == "metrics" && verb == "capital-return") {
        return Err(USAGE.into());
    }
    let mut fields = BTreeMap::new();
    for pair in args[2..].chunks(2) {
        let [flag, value] = pair else {
            return Err(format!("{VERB} requires a value for every option. {USAGE}"));
        };
        if !matches!(
            flag.as_str(),
            "--input" | "--issuer" | "--document" | "--series" | "--period" | "--as-of"
        ) || value.trim().is_empty()
            || value.starts_with("--")
            || fields.insert(flag.as_str(), value.as_str()).is_some()
        {
            return Err(format!(
                "{VERB} has an unknown, empty or duplicate {flag}. {USAGE}"
            ));
        }
    }
    if let Some(path) = fields.remove("--input") {
        return if fields.is_empty() {
            Ok(Command::Worksheet(path.into()))
        } else {
            Err(format!(
                "{VERB} worksheet and filing options are mutually exclusive. {USAGE}"
            ))
        };
    }
    let get = |key| {
        fields
            .get(key)
            .copied()
            .ok_or_else(|| format!("{VERB} requires {key}. {USAGE}"))
    };
    let issuer = get("--issuer")?;
    let series = get("--series")?;
    let as_of = get("--as-of")?;
    if !is_issuer_name(issuer) || !source::valid_series(series) || !is_date(as_of) {
        return Err(format!(
            "{VERB} requires a valid issuer, series key and calendar evidence date. {USAGE}"
        ));
    }
    let period = worksheet::period(get("--period")?).map_err(|reason| reason.what)?;
    Ok(Command::Filing {
        issuer: issuer.into(),
        document: get("--document")?.into(),
        series: series.into(),
        period,
        as_of: as_of.into(),
    })
}

fn filing(
    root: &Path,
    issuer: &str,
    document: &str,
    series: &str,
    period: Period,
    as_of: &str,
) -> Result<(CapitalReturnReceipt, String), Refusal> {
    let profile = load_issuer_profile(root, issuer)?;
    let file = profile
        .documents
        .iter()
        .find(|file| file.file == document)
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "capital return requires a selected registered filing",
            )
        })?;
    if file.published_at.as_str() > as_of {
        return Err(Refusal::new(
            RefusalCode::AssayLookahead108,
            format!(
                "capital return filing {document} was published after the selected evidence cut"
            ),
        ));
    }
    let selected = load_transcription_quantities_as_of(
        &profile,
        root,
        as_of,
        &source::quantities(series),
        Some(document),
    )?;
    let evidence = selected.filter(|report| {
        report.agreement.single_reader.is_empty()
            && report.documents.iter().any(|checked| checked.file == document && checked.sha256 == file.sha256)
    }).ok_or_else(|| {
        Refusal::new(RefusalCode::NotObservable202, "capital return needs selected source rows with checked document bytes and complete reader agreement")
    })?;
    let inputs = source::bind(issuer, series, period, file, &evidence.agreement.accepted)?;
    let receipt = capital_return::evaluate(&inputs)?;
    let selection = format!(
        r#""mode":"registered_filing","as_of":{},"series":{},"issuer":{},"document":{{"file":{},"sha256":{},"published_at":{},"published_at_basis":{}}}"#,
        js(as_of),
        js(series),
        js(issuer),
        js(document),
        js(&file.sha256),
        js(&file.published_at),
        js(&file.published_at_basis),
    );
    Ok((receipt, selection))
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (receipt, input, note) = match command {
            Command::Worksheet(path) => {
                let (text, anchor) = worksheet_input::read(
                    &root.join(path),
                    "capital return worksheet",
                    worksheet::MAX_BYTES as u64,
                )?;
                let input = format!(
                    r#""mode":"declared_worksheet","source":{}"#,
                    worksheet_input::source_json(&anchor, text.len()),
                );
                (
                    worksheet::evaluate(&text, anchor)?,
                    input,
                    "Declared worksheet inputs carry Assumed and Internal evidence.",
                )
            }
            Command::Filing {
                issuer,
                document,
                series,
                period,
                as_of,
            } => {
                let (receipt, input) = filing(root, issuer, document, series, *period, as_of)?;
                let note = if issuer == fmine_public::synthetic::SYNTHETIC_ISSUER {
                    "Synthetic filing example with invented inputs."
                } else {
                    "Source inputs carry agreed, registered issuer assertions."
                };
                (receipt, input, note)
            }
        };
        let payload = format!(
            r#"{{"schema":"fmine.capital-return-cli.v1",{input},"receipt":{}}}"#,
            receipt.to_json(),
        );
        let mut envelope = Envelope::ok(VERB, payload, Vec::new());
        envelope.no_claim = Some(capital_return::NO_CLAIM);
        envelope.warnings.push(note.into());
        envelope.rendered_markdown = Some(format!("{note}\n\n{}", receipt.to_markdown()));
        Ok(envelope)
    };
    run().unwrap_or_else(|reason| {
        Envelope::refused(
            VERB,
            reason,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect the worksheet and registered filing selectors",
            )],
        )
    })
}
