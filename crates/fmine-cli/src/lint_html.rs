//! Disclosure equality checks over an explicitly selected registered HTML population.
//! Publication evidence describes source files under today's registry; it never dates the mapping.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use fmine_core::Digest32;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_public::html::HtmlClaimsReport;
use fmine_public::key::KeyedCell;
use fmine_public::lint::{LintFinding, LintReport};
use fmine_public::sources::is_issuer_name;
use fmine_registry::sources::is_date;

use crate::{Envelope, NextAction, escape_json};

pub(super) const NO_CLAIM: &str = "Checks supported equalities against the selected registered HTML Claims. Source publication dates come from the current registry; historical availability of the mapping and registry is not established. Credit treatment is not inferred from AISC/TCC labels. This does not establish the truth of issuer assertions, verify other prose, authenticate a historical information set or authorize disclosure.";

fn js(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), js)
}

fn source_date(cell: &KeyedCell, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let projections: Vec<_> = snapshot
        .claims
        .iter()
        .filter(|claim| claim.cell.claim.claim_id == cell.claim.claim_id)
        .collect();
    let [projection] = projections.as_slice() else {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint finding does not identify one Claim in its verified projection",
        ));
    };
    let value_anchor = &projection.value_source.anchor;
    // Registry digests are unique; an in-workspace symlink may give the value anchor a different basename.
    let matches: Vec<_> = snapshot
        .sources
        .iter()
        .filter(|source| {
            source.sha256 == cell.key.reporting_vintage.file_sha256
                && source.release == cell.key.reporting_vintage.release
                && cell.claim.anchors.iter().any(|anchor| {
                    anchor == value_anchor
                        && Digest32(anchor.artifact_sha256).to_hex() == source.sha256
                })
        })
        .collect();
    let [source] = matches.as_slice() else {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint finding does not identify one source in its verified projection",
        ));
    };
    if snapshot
        .as_of
        .as_deref()
        .is_some_and(|cut| source.published_at.as_str() > cut)
    {
        return Err(Refusal::new(
            RefusalCode::FrozenRecordMutation507,
            "HTML lint projection contains a source after its publication cut",
        ));
    }
    Ok(format!(
        "{{\"claim_id\":{},\"file\":{},\"sha256\":{},\"release\":{},\"url\":{},\"published_at\":{},\"published_at_basis\":{},\"availability\":{}}}",
        js(&cell.claim.claim_id.to_string()),
        js(&source.file),
        js(&source.sha256),
        js(&source.release),
        js(&source.url),
        js(&source.published_at),
        js(&source.published_at_basis),
        js(if snapshot.as_of.is_some() {
            "published_by_cut_under_current_registry"
        } else {
            "registered_publication_date"
        }),
    ))
}

fn finding_dates(finding: &LintFinding, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let mut ids = BTreeSet::new();
    let sources = finding
        .source
        .iter()
        .chain(&finding.candidates)
        .chain(&finding.later)
        .filter(|cell| ids.insert(cell.claim.claim_id))
        .map(|cell| source_date(cell, snapshot))
        .collect::<Result<Vec<_>, _>>()?;
    let status = if sources.is_empty() {
        "no_source_resolved_in_selected_population"
    } else if snapshot.as_of.is_some() {
        "resolved_sources_published_by_cut"
    } else {
        "resolved_source_dates_recorded"
    };
    Ok(format!(
        "{{\"span\":{{\"start_byte\":{},\"end_byte\":{}}},\"equality_verdict\":{},\"status\":{},\"sources\":[{}]}}",
        finding.span.0,
        finding.span.1,
        js(finding.verdict.as_str()),
        js(status),
        sources.join(","),
    ))
}

fn publication_json(lint: &LintReport, snapshot: &HtmlClaimsReport) -> Result<String, Refusal> {
    let findings = lint
        .findings
        .iter()
        .map(|finding| finding_dates(finding, snapshot))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    Ok(format!(
        "{{\"schema\":\"fmine.html_source_publication.v1\",\"as_of\":{},\"scope\":\"registered_source_availability_separate_from_equality\",\"registry_basis\":\"current_captured_registry\",\"mapping_historical_availability\":\"not_established\",\"registry_historical_availability\":\"not_established\",\"excluded_sources\":{},\"excluded_source_bytes_read\":false,\"unresolved_citations\":\"An absent Claim is unresolved in this selection; its absence does not establish that the assertion was unpublished\",\"findings\":[{findings}]}}",
        optional(snapshot.as_of.as_deref()),
        snapshot.excluded_sources.len(),
    ))
}

pub(super) fn execute(
    path: &Path,
    issuer: &str,
    definitions: &Path,
    discover: bool,
    as_of: Option<&str>,
    root: &Path,
) -> Envelope {
    let actions = || vec![NextAction::new("fmine robot-docs", crate::ROBOT_DOCS_WHY)];
    if !is_issuer_name(issuer)
        || issuer == "synthetic"
        || definitions.as_os_str().is_empty()
        || as_of.is_some_and(|cut| !is_date(cut))
    {
        return Envelope::usage_error(
            "lint",
            "HTML lint requires a registered issuer, an explicit HTML mapping and an optional valid YYYY-MM-DD publication cut",
        );
    }
    let run = || -> Result<Envelope, Refusal> {
        let draft = crate::lint::read_draft(&root.join(path))?;
        let snapshot = fmine_public::html::run(root, issuer, definitions, as_of)?;
        let cells: Vec<_> = snapshot
            .claims
            .iter()
            .map(|claim| claim.cell.clone())
            .collect();
        let lint = if discover {
            fmine_public::lint::lint_discover(&draft, &cells)
        } else {
            fmine_public::lint::lint_cited(&draft, &cells)
        };
        if lint.claims_digest != snapshot.claims_digest {
            return Err(Refusal::new(
                RefusalCode::FrozenRecordMutation507,
                "HTML lint changed the canonical source population",
            ));
        }
        let payload = format!(
            "{{\"schema\":\"fmine.lint_html_snapshot.v1\",\"issuer\":{},\"lane\":\"public\",\"read_mode\":\"verified_html_source_projection\",\"source_state_root\":null,\"as_of\":{},\"lint\":{},\"source_publication\":{},\"html_projection\":{}}}",
            js(issuer),
            optional(as_of),
            lint.to_json(),
            publication_json(&lint, &snapshot)?,
            crate::html::report_json(&snapshot)?,
        );
        let mut markdown = lint.to_markdown();
        let _ = writeln!(markdown,
            "\n## HTML source publication evidence\n\n{}\n\nPublication evidence is separate from the equality verdict. Historical availability of the mapping and registry is not established. Unresolved citations remain unresolved; no future source bytes are read.\n",
            as_of.map_or_else(|| "Registered source dates are reported retrospectively.".into(),
                |cut| format!("The selected HTML sources have registered publication dates on or before {cut}.")),
        );
        markdown.push_str(&crate::html::markdown(&snapshot));
        let mut envelope = Envelope::ok("lint", payload, actions());
        envelope.no_claim = Some(NO_CLAIM);
        envelope.rendered_markdown = Some(markdown);
        Ok(envelope)
    };
    run().unwrap_or_else(|refusal| Envelope::refused("lint", refusal, actions()))
}
