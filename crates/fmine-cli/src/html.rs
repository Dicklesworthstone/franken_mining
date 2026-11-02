//! Canonical public Claims from registered HTML bytes and explicit, source-checked label mappings.
//! Determinism: Exact

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fmine_core::{Digest32, anchor::SourceAnchor};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::html::{HtmlCellCitation, HtmlClaim, HtmlClaimsReport};
use fmine_public::key::PublicValue;
use fmine_public::sources::is_issuer_name;
use fmine_registry::sources::{HtmlSourceRow, is_date};

use crate::claim_inspection::canonical_claim_json;
use crate::guidance::text;
use crate::{Envelope, NextAction, escape_json};

pub const USAGE: &str =
    "html claims --issuer <id> --definitions <mapping.toml> [--as-of YYYY-MM-DD]";
pub const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.html_claims.v1","source_mode":"registered_html_bytes","claims":"Canonical Claims, exact decimal tokens and source/label/mapping citations","as_of":"Optional inclusive registered-publication cut applied before source reads","excluded_sources":"Mapped sources published after the cut; their bytes are not read","authority":"No ledger anchor or certificate is created"}"#;
pub const NO_CLAIM: &str = "Source values remain issuer assertions and placeholders remain unknown. This projection uses the current registered source metadata and explicit label mappings; registered publication dates are not independently authenticated historical timestamps. No physical-truth, reserve-disclosure, regulatory-compliance or issuer-affiliation claim.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub issuer: String,
    pub definitions: PathBuf,
    pub as_of: Option<String>,
}

/// Common output/workspace flags are removed by the main router.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("html")
        || args.get(1).map(String::as_str) != Some("claims")
    {
        return Err(USAGE.into());
    }
    let mut values = BTreeMap::new();
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].as_str();
        if !matches!(flag, "--issuer" | "--definitions" | "--as-of") {
            return Err(format!("html claims: unsupported option {flag}; {USAGE}"));
        }
        let value = args
            .get(i + 1)
            .filter(|value| !value.trim().is_empty() && !value.starts_with("--"))
            .ok_or_else(|| format!("html claims: missing value for {flag}"))?;
        if values.insert(flag, value.as_str()).is_some() {
            return Err(format!("html claims: duplicate {flag}"));
        }
        i += 2;
    }
    let issuer = values
        .get("--issuer")
        .filter(|issuer| is_issuer_name(issuer) && **issuer != "synthetic")
        .ok_or_else(|| "html claims requires a registered --issuer".to_string())?;
    let definitions = values
        .get("--definitions")
        .ok_or_else(|| "html claims requires --definitions <mapping.toml>".to_string())?;
    let as_of = values.get("--as-of").copied();
    if as_of.is_some_and(|cut| !is_date(cut)) {
        return Err("html claims --as-of needs a valid YYYY-MM-DD date".into());
    }
    Ok(Command {
        issuer: (*issuer).into(),
        definitions: PathBuf::from(*definitions),
        as_of: as_of.map(str::to_string),
    })
}

pub fn execute(command: &Command, root: &Path) -> Envelope {
    let actions = vec![NextAction::new(
        "fmine robot-docs",
        "Inspect the registered HTML Claim command and source-selection requirements.",
    )];
    let result = if !is_issuer_name(&command.issuer)
        || command.issuer == "synthetic"
        || command.definitions.as_os_str().is_empty()
        || command.as_of.as_deref().is_some_and(|cut| !is_date(cut))
    {
        Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "HTML Claims require a registered issuer, definitions path and optional valid publication cut",
        ))
    } else {
        fmine_public::html::run(
            root,
            &command.issuer,
            &command.definitions,
            command.as_of.as_deref(),
        )
        .and_then(|report| Ok((report_json(&report)?, markdown(&report))))
    };
    match result {
        Ok((json, rendered)) => {
            let mut envelope = Envelope::ok("html claims", json, actions);
            envelope.no_claim = Some(NO_CLAIM);
            envelope.rendered_markdown = Some(rendered);
            envelope
        }
        Err(refusal) => Envelope::refused("html claims", refusal, actions),
    }
}

fn js(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn anchor(source: &SourceAnchor) -> String {
    format!(
        "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
        js(&source.artifact_path),
        js(&Digest32(source.artifact_sha256).to_hex()),
        js(&source.locator)
    )
}

fn citation(source: &HtmlCellCitation) -> String {
    format!(
        "{{\"anchor\":{},\"requested_locator\":{},\"byte_range\":{{\"start\":{},\"end_exclusive\":{}}},\"text\":{},\"row_span\":{},\"column_span\":{}}}",
        anchor(&source.anchor),
        js(&source.requested_locator),
        source.byte_range.0,
        source.byte_range.1,
        js(&source.text),
        source.row_span,
        source.column_span
    )
}

fn source(row: &HtmlSourceRow) -> String {
    format!(
        "{{\"file\":{},\"dir\":{},\"release\":{},\"url\":{},\"url_basis\":{},\"published_at\":{},\"published_at_basis\":{},\"bytes\":{},\"sha256\":{}}}",
        js(&row.file),
        js(&row.dir),
        js(&row.release),
        js(&row.url),
        js(&row.url_basis),
        js(&row.published_at),
        js(&row.published_at_basis),
        row.bytes,
        js(&row.sha256)
    )
}

fn html_claim(value: &HtmlClaim) -> Result<String, Refusal> {
    let exact = value.exact_number.as_ref().map_or_else(
        || "null".into(),
        |number| format!(
            "{{\"raw\":{},\"value\":{{\"mantissa\":{},\"scale\":{}}},\"step\":{{\"mantissa\":{},\"scale\":{}}},\"percent\":{},\"paren_negative\":{}}}",
            js(&number.raw), js(&number.value.mantissa.to_string()), number.value.scale,
            js(&number.step.mantissa.to_string()), number.step.scale, number.percent,
            number.paren_negative
        ),
    );
    let interpretations = value
        .interpretations
        .iter()
        .map(|meaning| {
            format!(
                "{{\"role\":{},\"value\":{},\"source\":{}}}",
                js(meaning.role),
                js(&meaning.value),
                citation(&meaning.source)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "{{\"definition_id\":{},\"claim\":{},\"exact_number\":{exact},\"value_source\":{},\"interpretations\":[{interpretations}],\"mapping_source\":{}}}",
        js(&value.definition_id),
        canonical_claim_json(&value.cell.claim, "html_source", value.cell.to_json())?,
        citation(&value.value_source),
        anchor(&value.mapping_source)
    ))
}

pub(super) fn report_json(report: &HtmlClaimsReport) -> Result<String, Refusal> {
    let claims = report
        .claims
        .iter()
        .map(html_claim)
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.html_claims.v1\",\"issuer\":{},\"source_mode\":\"registered_html_bytes\",\"gate_eligible\":false,\"as_of\":{},\"cut_semantics\":{},\"definitions\":{{\"path\":{},\"sha256\":{}}},\"registry_sha256\":{},\"claims_digest\":{},\"sources\":[{}],\"excluded_sources\":[{}],\"claims\":[{claims}],\"no_claim\":{}}}",
        js(&report.issuer),
        report.as_of.as_deref().map_or("null".into(), js),
        js(if report.as_of.is_some() {
            "inclusive_registered_publication_date"
        } else {
            "retrospective_current_registry"
        }),
        js(&report.definitions_path),
        js(&report.definitions_sha256),
        js(&report.registry_sha256),
        js(&report.claims_digest.to_hex()),
        report
            .sources
            .iter()
            .map(source)
            .collect::<Vec<_>>()
            .join(","),
        report
            .excluded_sources
            .iter()
            .map(source)
            .collect::<Vec<_>>()
            .join(","),
        js(NO_CLAIM)
    ))
}

pub(super) fn markdown(report: &HtmlClaimsReport) -> String {
    let mut out = format!(
        "# HTML Claims — {}\n\n{}\n\nDefinitions: {}; SHA-256 `{}`. Source registry SHA-256 `{}`.\n\n",
        text(&report.issuer),
        report.as_of.as_ref().map_or_else(
            || "Retrospective projection from the current registered sources and mappings.".into(),
            |cut| format!(
                "Sources published on or before {} under their registered publication dates.",
                text(cut)
            )
        ),
        text(&report.definitions_path),
        report.definitions_sha256,
        report.registry_sha256
    );
    for value in &report.claims {
        let cell = &value.cell;
        let unit = value
            .interpretations
            .iter()
            .find(|meaning| meaning.role == "unit")
            .map_or(cell.key.unit.as_str(), |meaning| {
                meaning.source.text.as_str()
            });
        let ownership = value
            .interpretations
            .iter()
            .find(|meaning| meaning.role == "ownership")
            .map_or("", |meaning| meaning.source.text.as_str());
        let shown = match &cell.claim.value {
            PublicValue::Number { stored, .. } => {
                format!("{} {}", text(stored), text(unit))
            }
            PublicValue::Nil { token, .. } => format!("Not reported ({})", text(token)),
            PublicValue::NotApplicable { token } => format!("Not applicable ({})", text(token)),
            PublicValue::Missing => "Missing".into(),
        };
        let _ = writeln!(
            out,
            "## {} — {}\n\n{}; reporting release {}. **{shown}**.\n\nClaim `{}`; {:?}, {:?}; ownership {}.\n\nSource: {} / {}; SHA-256 `{}`; byte range {}..{}.\n\n| Interpretation | Verified label | Source locator |\n|---|---|---|",
            text(&cell.key.subject),
            text(
                cell.label_as_printed
                    .as_deref()
                    .unwrap_or(&cell.key.line.label)
            ),
            cell.key.valid_period.code(),
            text(&cell.key.reporting_vintage.release),
            cell.claim.claim_id,
            cell.claim.evidence,
            cell.claim.state,
            text(ownership),
            text(&value.value_source.anchor.artifact_path),
            text(&value.value_source.anchor.locator),
            Digest32(value.value_source.anchor.artifact_sha256).to_hex(),
            value.value_source.byte_range.0,
            value.value_source.byte_range.1
        );
        for meaning in &value.interpretations {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                text(meaning.role),
                text(&meaning.source.text),
                text(&meaning.source.anchor.locator)
            );
        }
        let _ = writeln!(
            out,
            "\nMapping row: {} / {}.\n",
            text(&value.mapping_source.artifact_path),
            text(&value.mapping_source.locator)
        );
    }
    if !report.excluded_sources.is_empty() {
        out.push_str("## Sources after the publication cut\n\nTheir bytes were not read.\n\n");
        for source in &report.excluded_sources {
            let _ = writeln!(
                out,
                "- {} — published {}.",
                text(&source.file),
                text(&source.published_at)
            );
        }
    }
    let _ = writeln!(
        out,
        "\nClaims digest: `{}`. JSON retains complete canonical Claim bodies, exact decimal tokens and all source citations.\n\n{NO_CLAIM}",
        report.claims_digest.to_hex()
    );
    out
}
