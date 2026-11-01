//! Cited table inspection: existing workbook identities and explicitly declared PDF roll-forwards.
//! Determinism: Exact (registered bytes, sorted declarations, exact solver and stable rendering).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::roll_forward::{self, TableReport};
use fmine_numerics::exact_feas::{BlameOutcome, CorrectionLabel, Interval, Outcome};
use fmine_public::sources::{IssuerProfile, load_issuer_profile, transcriptions_path};
use fmine_public::transcribe::{check_docs, load_transcription_quantities_as_of};
use fmine_public::{PublicAssuranceReport, run_public_assurance, run_synthetic_assurance};
use fmine_registry::table_identities::parse_table_identities;

use crate::guidance::{number, text};
use crate::{Envelope, NextAction, escape_json};

const NO_CLAIM: &str = "Checks arithmetic under explicitly declared identities, membership and rounding rules. \
    Source values remain issuer assertions. A conflict does not select a corrected value or establish physical \
    truth, a reserve classification, regulatory compliance or issuer affiliation.";

fn js(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine doctor",
        "inspect registered source availability",
    )]
}

pub(super) fn execute(
    root: &Path,
    issuer: Option<&str>,
    through: Option<&str>,
    definitions: Option<&Path>,
) -> Envelope {
    let result = match issuer {
        Some(issuer) => load_issuer_profile(root, issuer).and_then(|profile| {
            if let Some(path) = definitions {
                if through.is_some() {
                    return Err(Refusal::new(
                        RefusalCode::NotObservable202,
                        "PDF definitions and workbook --through cannot be combined",
                    ));
                }
                document_tables(root, &profile, path)
            } else {
                let profile = match through {
                    Some(bound) => profile.through(bound)?,
                    None => profile,
                };
                run_public_assurance(&profile, root).map(|r| workbook_envelope(&r))
            }
        }),
        None if through.is_none() && definitions.is_none() => {
            run_synthetic_assurance(&[]).map(|r| workbook_envelope(&r))
        }
        None => {
            return Envelope::usage_error("check tables", "source selectors require --issuer");
        }
    };
    result.unwrap_or_else(|r| Envelope::refused("check tables", r, actions()))
}

fn document_tables(root: &Path, profile: &IssuerProfile, path: &Path) -> Result<Envelope, Refusal> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let bytes = std::fs::read(&path).map_err(|e| {
        Refusal::new(
            RefusalCode::InputMissingOrUnreadable506,
            format!("table definitions {}: {e}", path.display()),
        )
    })?;
    let source = std::str::from_utf8(&bytes).map_err(|e| {
        Refusal::new(
            RefusalCode::NotObservable202,
            format!("table definitions {}: {e}", path.display()),
        )
    })?;
    let definitions =
        parse_table_identities(source, &profile.issuer, &profile.documents).map_err(|e| {
            Refusal::new(
                RefusalCode::NotObservable202,
                format!("table definitions {}: {e}", path.display()),
            )
        })?;

    // Verify even a declared document with no transcribed rows: an absent member must not hide absent bytes.
    let quantities = definitions.quantities_by_document();
    let documents: Vec<_> = profile
        .documents
        .iter()
        .filter(|d| quantities.contains_key(d.file.as_str()))
        .collect();
    check_docs(&documents, root)?;
    let mut accepted = Vec::new();
    let mut pending = Vec::new();
    for (doc, keys) in &quantities {
        // This is retrospective inspection of named documents, not an inferred information cut.
        let Some(report) =
            load_transcription_quantities_as_of(profile, root, "9999-12-31", keys, Some(doc))?
        else {
            return Err(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!(
                    "declared table values require {}",
                    transcriptions_path(root, &profile.issuer).display()
                ),
            ));
        };
        let clashes = report.agreement.accepted_pending_clashes();
        if !clashes.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                clashes.join("; "),
            ));
        }
        accepted.extend(report.agreement.accepted);
        pending.extend(report.agreement.single_reader);
    }
    accepted.sort_by(|a, b| a.locator.cmp(&b.locator));
    pending.sort();
    let report = roll_forward::assess(&definitions, &accepted, &profile.documents);
    let reserves = fmine_metrics::reserves::assess(&definitions, &accepted, &profile.documents);
    let pending_json = pending
        .iter()
        .map(|(at, reader)| {
            format!(
                "{{\"locator\":{},\"reader\":{}}}",
                js(&at.to_string()),
                js(reader),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        "{{\"schema\":\"fmine.table-check.v1\",\"mode\":\"declared_pdf\",\"lane\":\"public\",\"gate_eligible\":false,\
        \"issuer\":{},\"definitions_sha256\":{},\"checks\":{},\"issuer_reserves\":{},\"pending_readings\":[{}],\"no_claim\":{}}}",
        js(&profile.issuer),
        js(&sha256(&bytes).to_hex()),
        report.to_json(),
        reserves.to_json(),
        pending_json,
        js(NO_CLAIM),
    );
    let digest = sha256(format!("fmine.table-check/1\n{body}").as_bytes()).to_hex();
    let payload = format!("{{\"digest\":{},{}", js(&digest), &body[1..]);
    let mut envelope = Envelope::ok("check tables", payload, actions());
    envelope.no_claim = Some(NO_CLAIM);
    envelope.rendered_markdown = Some(
        document_markdown(&profile.issuer, &report, &pending, &digest) + &reserves.to_markdown(),
    );
    Ok(envelope)
}

fn outcome(value: &Outcome) -> &'static str {
    match value {
        Outcome::Feasible { .. } => "feasible",
        Outcome::Infeasible { .. } => "infeasible",
        Outcome::NotRefuted => "not_refuted",
        Outcome::Indeterminate { .. } => "indeterminate",
    }
}

fn exact_interval(value: Interval) -> String {
    format!(
        "{}{}, {}{}",
        if value.lo_strict { '(' } else { '[' },
        number(value.lo),
        number(value.hi),
        if value.hi_strict { ')' } else { ']' }
    )
}

fn document_markdown(
    issuer: &str,
    report: &TableReport,
    pending: &[(fmine_registry::transcriptions::Locator, String)],
    digest: &str,
) -> String {
    let mut out = format!(
        "# Table checks: {}\n\n{}\n\nReport digest: `{digest}`\n\n",
        text(issuer),
        NO_CLAIM
    );
    out.push_str("## Declared identities\n\n| Identity / document | Family / kind | Total (period) | Signed members (period, role) | Unit / basis | Construction / convention |\n|---|---|---|---|---|---|\n");
    for d in &report.definitions {
        let terms = d
            .terms
            .iter()
            .map(|t| {
                format!(
                    "{} × {} ({}, {})",
                    t.sign,
                    t.quantity,
                    t.period,
                    t.role.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            out,
            "| {} / {} | {} / {} | {} ({}) | {} | {} / {} | {} / {} |",
            text(&d.id),
            text(&d.doc),
            text(&d.family),
            d.kind.as_str(),
            text(&d.total),
            text(&d.total_period),
            text(&terms),
            text(&d.unit),
            text(&d.basis),
            text(&d.construction),
            text(&d.convention)
        );
    }
    out.push_str(
        "\n## Joint results\n\n| Identities | Result | Checked certificate |\n|---|---|---|\n",
    );
    for check in &report.components {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            text(&check.component.identities.join(", ")),
            outcome(&check.component.outcome),
            check
                .certificate_digest
                .as_deref()
                .map_or_else(|| "—".to_string(), |d| format!("`{d}`")),
        );
        if let Outcome::Indeterminate { reason } = &check.component.outcome {
            let _ = writeln!(out, "\n{}\n", text(reason));
        }
    }
    for check in &report.components {
        let Some(blame) = &check.blame else {
            continue;
        };
        let _ = writeln!(
            out,
            "\n### Conflict detail: {}\n",
            text(&check.component.identities.join(", "))
        );
        match blame {
            BlameOutcome::Blamed(blame) => {
                let label = match blame.label {
                    CorrectionLabel::Minimum => "All minimum correction sets",
                    CorrectionLabel::InclusionMinimalOnly => {
                        "Inclusion-minimal correction set; minimum size not proved"
                    }
                };
                let _ = writeln!(
                    out,
                    "{label}. These identify printed cells whose bounds can be relaxed together; they do not prescribe corrected values.\n"
                );
                for (n, correction) in blame.corrections.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "Set {}: {}.\n",
                        n + 1,
                        text(&correction.cells.join(", "))
                    );
                    for (cell, implied) in &correction.implied {
                        let range = match implied {
                            Ok(interval) => exact_interval(*interval),
                            Err(reason) => format!("indeterminate: {reason:?}"),
                        };
                        let _ = writeln!(out, "- {}: {}", text(cell), text(&range));
                    }
                    out.push('\n');
                }
            }
            BlameOutcome::Indeterminate { reason } => {
                let _ = writeln!(out, "Correction search indeterminate: {}\n", text(reason));
            }
        }
    }
    if !report.refused.is_empty() {
        out.push_str(
            "\n## Unresolved identities\n\n| Identity | Document | Reason |\n|---|---|---|\n",
        );
        for refused in &report.refused {
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                text(&refused.identity),
                text(&refused.doc),
                text(&refused.reason)
            );
        }
    }
    out.push_str("\n## Source readings\n\n| Quantity | Printed token | Period | Unit / basis | Locator | Published / kind | Readers | SHA-256 |\n|---|---|---|---|---|---|---|---|\n");
    for cell in &report.cells {
        let a = &cell.source;
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} / {} | {} | {} / {} | {} | `{}` |",
            text(&a.quantity),
            text(&a.token),
            text(&a.period),
            text(&a.unit),
            text(&a.basis),
            text(&a.locator.to_string()),
            text(&cell.document.published_at),
            text(&cell.document.kind),
            text(&a.readers.join(", ")),
            a.doc_sha256,
        );
    }
    let mut rejected = BTreeMap::new();
    for refusal in &report.refused {
        for reading in &refusal.evidence {
            if !report.cells.iter().any(|c| c.source == *reading) {
                rejected.insert(reading.locator.clone(), reading);
            }
        }
    }
    if !rejected.is_empty() {
        out.push_str("\n## Evidence retained for unresolved identities\n\n| Quantity | Printed token | Period | Unit / basis | Locator | Readers | SHA-256 |\n|---|---|---|---|---|---|---|\n");
        for reading in rejected.values() {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} / {} | {} | {} | `{}` |",
                text(&reading.quantity),
                text(&reading.token),
                text(&reading.period),
                text(&reading.unit),
                text(&reading.basis),
                text(&reading.locator.to_string()),
                text(&reading.readers.join(", ")),
                reading.doc_sha256
            );
        }
    }
    if !pending.is_empty() {
        out.push_str("\n## Pending independent readings\n\n");
        for (locator, reader) in pending {
            let _ = writeln!(
                out,
                "- {}: one reading by {}",
                text(&locator.to_string()),
                text(reader)
            );
        }
    }
    out
}

fn workbook_envelope(report: &PublicAssuranceReport) -> Envelope {
    // Reuse the authoritative workbook ledger and its exact certificate; never reconstruct a second verdict.
    let payload = format!(
        "{{\"schema\":\"fmine.table-check.v1\",\"mode\":\"workbooks\",\"lane\":{},\
         \"assurance\":{},\"no_claim\":{}}}",
        js(report.lane.as_str()),
        report.to_json(),
        js(NO_CLAIM),
    );
    let mut envelope = Envelope::report(
        "check tables",
        report.ledger_anchor.anchor_id().to_string(),
        report.ledger_anchor.state_root.to_hex(),
        payload,
        actions(),
    );
    envelope.no_claim = Some(NO_CLAIM);
    let mut out = format!(
        "# Table checks: {}\n\nLane: **{}**.\n\n{}\n\n",
        text(&report.issuer),
        report.lane.as_str(),
        NO_CLAIM
    );
    out.push_str("## Workbook identity states\n\n| Source / subject | Family | State | Detail |\n|---|---|---|---|\n");
    for i in &report.instances {
        let _ = writeln!(
            out,
            "| {} / {} | {} | {} | {} |",
            text(&format!("{}:{}!{}", i.file, i.sheet, i.cell)),
            text(&i.subject),
            text(i.family.as_str()),
            text(i.state.label()),
            text(&i.state.detail())
        );
    }
    out.push_str("\n## Mine Stats identity states\n\n| Source / subject | Field | State | Component |\n|---|---|---|---|\n");
    for i in &report.mine_ledger.instances {
        let _ = writeln!(
            out,
            "| {} / {} | {} | {} | {} |",
            text(&i.anchor),
            text(&i.subject),
            text(&i.field),
            i.state.code(),
            text(i.component.as_deref().unwrap_or("—"))
        );
    }
    for c in &report.mine_ledger.infeasible {
        let _ = writeln!(
            out,
            "\n### Shared-cell conflict: {}\n\nJoint-only: {}. Exhaustive minimum search: {}.\n",
            text(&c.id),
            c.joint_only,
            c.minimum
        );
        for correction in &c.corrections {
            let _ = writeln!(
                out,
                "\nCells whose printed bounds must be relaxed together: {}.\n",
                text(&correction.cells.join(", "))
            );
            for (cell, range) in &correction.implied {
                let range = match range {
                    Ok(v) => exact_interval(*v),
                    Err(reason) => reason.clone(),
                };
                let _ = writeln!(out, "- {}: {}", text(cell), text(&range));
            }
        }
    }
    out.push_str("\n## Verified workbook bytes\n\n| File | SHA-256 |\n|---|---|\n");
    for source in &report.sources {
        let _ = writeln!(
            out,
            "| {} | `{}` |",
            text(&source.file_name),
            fmine_core::hash::Digest32(source.workbook.sha256).to_hex()
        );
    }
    envelope.rendered_markdown = Some(out);
    envelope
}
