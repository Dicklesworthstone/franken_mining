//! The readable M0 report over the already assessed public ledger (BD-072).
//! Determinism: Exact
//! Rendering preserves source strings and every selected vintage; it chooses no true assertion.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fmine_core::Digest32;
use fmine_evidence::Refusal;
use fmine_public::aisc::{IdentityFamily, IdentityInstance};
use fmine_public::consistency::{Consistency, ConsistencyFamily, FamilyCounts};
use fmine_public::key::consistency::{Comparison, VintageReport};
use fmine_public::key::{KeyedCell, PublicValue};
use fmine_public::mine_ledger::{State, rat_text};
use fmine_public::{Lane, PublicAssuranceReport};

use crate::packet;

const STATES: [&str; 6] = ["i", "ii", "iii", "iv", "v", "np"];

fn text(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' | '\r' => out.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '|' | '#' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

fn row(out: &mut String, values: &[String]) {
    let _ = writeln!(
        out,
        "| {} |",
        values
            .iter()
            .map(|s| text(s))
            .collect::<Vec<_>>()
            .join(" | ")
    );
}

fn table(out: &mut String, headers: &[&str]) {
    out.push('\n');
    row(
        out,
        &headers.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
    );
    let _ = writeln!(out, "| {} |", vec!["---"; headers.len()].join(" | "));
}

fn shown(cell: &KeyedCell) -> String {
    match &cell.claim.value {
        PublicValue::Number {
            stored,
            displayed,
            under_status,
        } => {
            let mut value = match displayed {
                Ok(value) if value == stored => value.clone(),
                Ok(value) => format!("{value}; stored {stored}"),
                Err(reason) => format!("display refused ({reason}); stored {stored}"),
            };
            if let Some(status) = under_status {
                let _ = write!(value, "; status {status:?}");
            }
            value
        }
        PublicValue::Nil { token, reason } => format!("not reported ({token}); {reason:?}"),
        PublicValue::NotApplicable { token } => format!("not applicable ({token})"),
        PublicValue::Missing => "missing; no numeric observation".into(),
    }
}

fn source_inventory(out: &mut String, report: &PublicAssuranceReport) {
    out.push_str("## Verified workbook inputs\n\nRetrieval metadata is not a publication timestamp. Numeric-cell counts describe extraction, not independent agreement or a gate pass. Each source locator below resolves to this file and SHA-256 inventory.\n");
    table(
        out,
        &[
            "File / kind",
            "Release",
            "Bytes",
            "Visible / hidden sheets",
            "Numeric / placeholder / error cells",
            "SHA-256",
        ],
    );
    for source in &report.sources {
        let census = &source.census;
        row(
            out,
            &[
                format!("{} / {}", source.file_name, source.kind.as_str()),
                source.quarter.clone(),
                source.bytes.to_string(),
                format!("{} / {}", census.visible_sheets, census.hidden_sheets),
                format!(
                    "{} / {} / {}",
                    census.numeric_cells, census.placeholder_cells, census.formula_error_cells
                ),
                Digest32(source.workbook.sha256).to_hex(),
            ],
        );
    }
    table(out, &["File", "Source URL", "URL basis"]);
    for source in &report.sources {
        row(
            out,
            &[
                source.file_name.clone(),
                source.url.clone(),
                source.url_basis.clone(),
            ],
        );
    }
}

fn counts<F: ConsistencyFamily>(out: &mut String, heading: &str, values: &FamilyCounts<F>) {
    let _ = writeln!(out, "\n### {heading}\n");
    table(out, &["Family", "i", "ii", "iii", "iv", "v", "np"]);
    for &family in F::ALL {
        let mut values_row = vec![family.as_str().into()];
        values_row.extend(STATES.map(|state| {
            values
                .get(&(family, state))
                .copied()
                .unwrap_or(0)
                .to_string()
        }));
        row(out, &values_row);
    }
}

fn aisc_instances(out: &mut String, instances: &[IdentityInstance]) {
    table(
        out,
        &[
            "Subject",
            "Family",
            "State",
            "Source",
            "Recorded explanation / refusal",
        ],
    );
    for instance in instances {
        row(
            out,
            &[
                instance.subject.clone(),
                instance.family.as_str().into(),
                format!("{}: {}", instance.state.code(), instance.state.label()),
                format!("{}:{}!{}", instance.file, instance.sheet, instance.cell),
                instance.state.detail(),
            ],
        );
    }
}

fn ledger(out: &mut String, report: &PublicAssuranceReport) {
    out.push_str("\n## Identity accounting\n\nThe five states are **i** consistent under the declared construction; **ii** explained (stated and reconstructed remain distinct); **iii** issuer discrepancy; **iv** undecidable from public data; **v** refused. **np** is the separate not-published population and is never silently dropped. Consistency does not prove an issuer assertion true.\n");
    counts(out, "Visible AISC identities", &report.state_counts());
    let _ = writeln!(
        out,
        "\n{} located visible AISC tables; {} instances are relaxation-only checks that do not refute a weighted aggregation and prove no value.\n",
        report.aisc_tables,
        report.relaxation_only()
    );
    aisc_instances(out, &report.instances);
    counts(
        out,
        "Visible Mine Stats identities",
        &report.mine_stats_consistency.identities.all_files,
    );
    table(
        out,
        &[
            "Subject / field",
            "Period / family",
            "State",
            "Source",
            "Recorded context",
        ],
    );
    for instance in &report.mine_ledger.instances {
        let detail = match &instance.state {
            State::Unresolved(need) => format!("missing fact: {}", need.code()),
            State::Refused { reason, at } => format!(
                "{}; {}",
                reason.code(),
                at.as_deref().unwrap_or("no offending cell identified")
            ),
            State::NotPublished => "all operands are placeholders".into(),
            _ => format!(
                "group {}; share {}; joint component {}",
                instance.group.as_str(),
                instance
                    .share
                    .map_or_else(|| "not supplied".into(), rat_text),
                instance.component.as_deref().unwrap_or("none")
            ),
        };
        row(
            out,
            &[
                format!("{} / {}", instance.subject, instance.field),
                format!("{} / {}", instance.period.code(), instance.family.as_str()),
                instance.state.code().into(),
                instance.anchor.clone(),
                detail,
            ],
        );
    }
    let _ = writeln!(
        out,
        "\nJoint components: {}; infeasible: {}; individually consistent but jointly infeasible: {}. Joint conflicts do not rewrite individual states; their exact correction sets are retained in the evidence packets.\n",
        report.mine_ledger.components,
        report.mine_ledger.infeasible.len(),
        report
            .mine_ledger
            .infeasible
            .iter()
            .filter(|c| c.joint_only)
            .count()
    );
    for (at, reason) in &report.mine_ledger.indeterminate {
        let _ = writeln!(
            out,
            "- Joint assessment not decided at {}: {}.",
            text(at),
            text(reason)
        );
    }
    out.push_str(&report.mine_ledger.cost_layers.to_markdown());
}

fn metrics(out: &mut String, report: &PublicAssuranceReport) {
    out.push_str("\n## Published values and all selected vintages\n\nEvery row is an original canonical source Claim. Printed and stored strings are retained without floating-point conversion; placeholders and flagged figures remain distinct from usable numbers. No later release replaces an earlier assertion. Metric labels and ownership are not converted to another definition.\n");
    let mut groups = BTreeMap::new();
    for cell in report.public_claims.cells.values() {
        groups
            .entry((
                &cell.key.subject,
                cell.key.valid_period,
                &cell.key.scope,
                cell.key.ownership_basis,
            ))
            .or_insert_with(Vec::new)
            .push(cell);
    }
    for ((subject, period, scope, ownership), mut cells) in groups {
        cells.sort_by(|a, b| (&a.key, &a.locator).cmp(&(&b.key, &b.locator)));
        let _ = writeln!(
            out,
            "\n### {} — {}\n\nScope: {}; ownership: {}.\n",
            text(subject),
            period.code(),
            text(&format!("{scope:?}")),
            text(&format!("{ownership:?}"))
        );
        table(
            out,
            &[
                "Reporting vintage",
                "Metric / occurrence",
                "Printed value or status",
                "Unit",
                "Claim",
                "Source",
            ],
        );
        for cell in &cells {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | `{}` | {} |",
                text(&cell.key.reporting_vintage.release),
                text(&format!(
                    "{} / {}",
                    cell.label_as_printed
                        .as_deref()
                        .unwrap_or(&cell.key.line.label),
                    cell.key.line.occurrence
                )),
                text(&shown(cell)),
                text(&cell.key.unit),
                cell.claim.claim_id,
                text(&cell.locator),
            );
        }
        // Interpretation notes follow the complete table, so later rows keep their column headings.
        for cell in &cells {
            for (label, evidence) in [
                ("Basis", &cell.basis_evidence),
                ("Unit", &cell.unit_evidence),
                ("Status", &cell.status_evidence),
            ] {
                if let Some(evidence) = evidence {
                    let _ = writeln!(
                        out,
                        "\n{} interpretation for {}: {}.\n",
                        label,
                        cell.claim.claim_id,
                        text(evidence)
                    );
                }
            }
        }
    }
}

fn identity_history<F: ConsistencyFamily>(out: &mut String, title: &str, value: &Consistency<F>) {
    let _ = writeln!(
        out,
        "\n### {title}\n\n{}\n",
        fmine_public::consistency::FIRST_REGISTERED_CAVEAT
    );
    counts(
        out,
        "Earliest registered identities",
        &value.first_registered,
    );
    table(
        out,
        &[
            "Subject / period / family",
            "Basis / scope / unit",
            "Release",
            "State / transition",
            "Operands changed",
            "Source / SHA-256",
        ],
    );
    for history in &value.histories {
        for (index, appearance) in history.sequence.iter().enumerate() {
            let transition = history.transitions.iter().find(|t| t.to == index);
            row(
                out,
                &[
                    format!(
                        "{} / {} / {}",
                        history.series.subject,
                        history.series.valid_period.code(),
                        history.family.as_str()
                    ),
                    format!(
                        "{:?} / {:?} / {}",
                        history.series.ownership_basis, history.series.scope, history.series.unit
                    ),
                    appearance.release.clone(),
                    format!(
                        "{} / {}",
                        appearance.state,
                        transition.map_or("initial appearance", |t| t.kind.as_str())
                    ),
                    transition
                        .and_then(|t| t.operands_changed)
                        .map_or_else(|| "not assessed".into(), |changed| changed.to_string()),
                    format!("{} / {}", appearance.at, appearance.sha256),
                ],
            );
        }
    }
    for history in &value.histories {
        if !history.ambiguous_vintages.is_empty() {
            let _ = writeln!(
                out,
                "\nAmbiguous releases for {} / {}: {}. All assertions are retained.\n",
                text(&history.series.subject),
                history.series.valid_period.code(),
                text(&history.ambiguous_vintages.join(", "))
            );
        }
    }
    for unkeyed in &value.unkeyed {
        let _ = writeln!(
            out,
            "\nUnkeyed {} at {}: {}. It remains in the all-file count and is excluded from the earliest-registered count.\n",
            unkeyed.family.as_str(),
            text(&unkeyed.at),
            text(&unkeyed.reason)
        );
    }
}

fn comparison(out: &mut String, name: &str, value: &Comparison) {
    let _ = writeln!(
        out,
        "\n{name}: **{}**; reason: {}; printed-token glyph changed: {}.\n",
        text(value.state.as_str()),
        text(value.reason.unwrap_or("none recorded")),
        value.token_glyph_changed
    );
    for note in &value.disclosures {
        let _ = writeln!(
            out,
            "\nVerified note {} at {}; SHA-256 {}; release {}; applicable from {}. Scope: {}, {} through {}, {} occurrence {}, unit {}.\n\nQuoted source: {}\n\nEvidence: {}; URL: {}.\n",
            text(&note.note_id),
            text(&note.at),
            note.sha256,
            text(&note.source_release),
            text(&note.from_quarter),
            text(&note.scope.subject),
            text(&note.scope.period_from),
            text(&note.scope.period_to),
            text(&note.scope.line),
            note.scope.occurrence,
            text(&note.unit),
            text(&note.quoted_text),
            text(&note.evidence),
            text(&note.url)
        );
    }
}

fn vintages(out: &mut String, value: &VintageReport) {
    let _ = writeln!(
        out,
        "\n### {} source-value comparisons\n\n{} semantic series; {} multi-vintage series; {} release appearances; {} comparisons. Input Claim digest: `{}`. Disclosure coverage: {}; registry digest: `{}`.\n",
        text(value.scope.as_str()),
        value.series,
        value.multi_vintage,
        value.totals.appearances,
        value.totals.comparisons,
        value.input_claims_digest.to_hex(),
        match value.disclosure_registry_complete {
            Some(true) => "registered notes checked",
            Some(false) => "incomplete; failed note evidence remains explicit",
            None => "not assessed",
        },
        value
            .disclosure_registry_digest
            .map_or_else(|| "not assessed".into(), |d| d.to_hex())
    );
    table(out, &["Comparison state", "Appearance count"]);
    for (state, count) in &value.totals.states {
        row(out, &[state.as_str().into(), count.to_string()]);
    }
    for item in &value.items {
        comparison(out, "Against earliest registered", &item.against_first);
        if let Some(previous) = &item.against_previous {
            comparison(out, "Against immediately previous", previous);
        }
        table(
            out,
            &[
                "Role",
                "Subject / period",
                "Release / ownership / unit",
                "Printed value",
                "Claim / source",
            ],
        );
        for (role, cells) in [
            ("earliest registered", &item.first),
            ("previous", &item.previous),
            ("current", &item.current),
        ] {
            for cell in cells {
                row(
                    out,
                    &[
                        role.into(),
                        format!("{} / {}", cell.key.subject, cell.key.valid_period.code()),
                        format!(
                            "{} / {:?} / {}",
                            cell.key.reporting_vintage.release,
                            cell.key.ownership_basis,
                            cell.key.unit
                        ),
                        shown(cell),
                        format!("{} / {}", cell.claim.claim_id, cell.locator),
                    ],
                );
            }
        }
    }
}

fn extraction_limits(out: &mut String, report: &PublicAssuranceReport) {
    out.push_str("\n## Unusable or unclassified source evidence\n");
    table(out, &["Kind", "Source", "Reason / printed label"]);
    for refused in &report.table_refusals {
        row(
            out,
            &[
                "table not located".into(),
                format!("{}:{}", refused.file, refused.sheet),
                refused.reason.clone(),
            ],
        );
    }
    for header in &report.unregistered_headers {
        row(
            out,
            &[
                "unregistered subject header".into(),
                format!("{}:{}!{}", header.file, header.sheet, header.cell),
                header.header.clone(),
            ],
        );
    }
    for refused in &report.public_claims.refusals {
        row(
            out,
            &[
                "Claim keying refused".into(),
                refused.locator.clone(),
                refused.reason.clone(),
            ],
        );
    }
    for at in &report.public_claims.unregistered_all_zero {
        row(
            out,
            &[
                "unregistered all-zero column".into(),
                at.clone(),
                "no registered semantic subject; not promoted to a Claim".into(),
            ],
        );
    }
    for refused in &report.disclosures.refusals {
        row(
            out,
            &[
                format!("disclosure {}", refused.note_id),
                refused.at.clone(),
                format!("{}: {}", refused.refusal.code, refused.refusal.what),
            ],
        );
    }
}

fn derived_sections(out: &mut String, report: &PublicAssuranceReport) {
    out.push_str("\n## Descriptive AISC bridges\n\nThe source-bound waterfall digest is included in the identity-ledger root. Closure is arithmetic, not causal attribution. The table retains exact rational rate changes; factor bars and their rounding enclosures remain in JSON, without reallocating the printed-rate gap to a bar.\n");
    let _ = writeln!(
        out,
        "\nWaterfall digest: `{}`.\n",
        report.waterfall.digest().to_hex()
    );
    table(
        out,
        &[
            "Subject / periods",
            "Outcome",
            "Recomputed change USD/oz",
            "Change in printed-rate gap USD/oz",
            "Source Claim IDs / refusal",
        ],
    );
    for pair in &report.waterfall.pairs {
        let (delta, gap, detail) = match &pair.outcome {
            Ok(bridge) => (
                rat_text(bridge.gap().total_delta),
                rat_text(bridge.gap().printed_rate_gap),
                pair.inputs
                    .iter()
                    .map(|c| c.claim.claim_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            Err(refusal) => (
                "not computed".into(),
                "not computed".into(),
                format!("{}: {}", refusal.code, refusal.what),
            ),
        };
        row(
            out,
            &[
                format!(
                    "{} / {} to {}",
                    pair.subject,
                    pair.from.code(),
                    pair.to.code()
                ),
                pair.kind().into(),
                delta,
                gap,
                detail,
            ],
        );
    }
    out.push_str("\n## Additional reports outside the identity certificate\n\nPDF transcription and recovery results are separate assessments. Their presence does not enlarge what the ledger certificate establishes. Any separately appended published-cutoff report likewise retains its own source checks.\n");
    if let Some(transcriptions) = &report.transcriptions {
        let agreement = &transcriptions.agreement;
        let _ = writeln!(
            out,
            "\n### Agreed PDF assertions\n\nRegistry: {}; accepted-row digest: `{}`. Accepted: {}; awaiting a second reader: {}; reader disagreements: {}; duplicate quantity contexts: {}. Reader agreement is not verification of an issuer assertion.\n",
            text(&transcriptions.registry),
            agreement.digest(),
            agreement.accepted.len(),
            agreement.single_reader.len(),
            agreement.disagreements.len(),
            agreement.quantity_clashes.len()
        );
        table(
            out,
            &[
                "Document",
                "Publication date",
                "Publication basis",
                "SHA-256",
            ],
        );
        for document in &transcriptions.documents {
            row(
                out,
                &[
                    document.file.clone(),
                    document.published_at.clone(),
                    document.published_at_basis.clone(),
                    document.sha256.clone(),
                ],
            );
        }
        table(
            out,
            &[
                "Quantity / period",
                "Printed token",
                "Unit / basis",
                "Source",
                "SHA-256",
            ],
        );
        for accepted in &agreement.accepted {
            row(
                out,
                &[
                    format!("{} / {}", accepted.quantity, accepted.period),
                    accepted.token.clone(),
                    format!("{} / {}", accepted.unit, accepted.basis),
                    accepted.locator.to_string(),
                    accepted.doc_sha256.clone(),
                ],
            );
        }
        for (at, reader) in &agreement.single_reader {
            let _ = writeln!(
                out,
                "\nPending second reader at {} (reader {}).\n",
                text(&at.to_string()),
                text(reader)
            );
        }
    } else {
        out.push_str("\nPDF transcription assessment: not present in this run.\n");
    }
    if let Some(recovery) = &report.recovery {
        let _ = writeln!(
            out,
            "\n### Retrospective recovery comparison\n\nModel-report digest: `{}`; fit Claim: `{}`; {} retained quarters. Descriptive and uncalibrated; signed differences are not recoverable-metal estimates. The complete model inputs, rounding enclosures, outcomes and refusals remain in JSON.\n",
            recovery.digest().to_hex(),
            recovery.fit.claim_id,
            recovery.quarters.len()
        );
        table(
            out,
            &[
                "Period / release",
                "Model",
                "Gap in percentage points (exact printed-input result)",
                "Output Claim / refusal",
            ],
        );
        for quarter in &recovery.quarters {
            match &quarter.outcome {
                Ok(projection) => {
                    for (name, claim) in [
                        ("constant tails", &projection.constant_tails),
                        ("grade-dependent tails", &projection.grade_dependent_tails),
                    ] {
                        row(
                            out,
                            &[
                                format!("{} / {}", quarter.period.code(), quarter.release),
                                name.into(),
                                rat_text(claim.value.gap_pp.point),
                                claim.claim_id.to_string(),
                            ],
                        );
                    }
                }
                Err(refusal) => row(
                    out,
                    &[
                        format!("{} / {}", quarter.period.code(), quarter.release),
                        "not computed".into(),
                        "not computed".into(),
                        format!("{}: {}", refusal.code, refusal.what),
                    ],
                ),
            }
        }
    } else {
        out.push_str("\nRecovery comparison: not present in this run.\n");
    }
}

/// Render existing assessed values and packet evidence, without altering the report or its certificate.
pub(super) fn markdown(report: &PublicAssuranceReport) -> Result<String, Refusal> {
    let packets = packet::packets_from_report(report)?;
    let mut out = format!(
        "# Public assurance — {}\n\n{}\n\n{}\n\n- Lane: {}\n- Selected releases: {}\n- Retrieval metadata: {}\n- Ledger anchor: `{}`\n- Identity state root: `{}`\n- Public gate eligibility: {} (eligibility does not establish passage)\n\n",
        text(&report.issuer),
        fmine_public::NO_CLAIM,
        if report.lane == Lane::Synthetic {
            "**SYNTHETIC: invented workbooks; no real issuer coverage and no gate eligibility.**"
        } else {
            packet::NO_AFFILIATION
        },
        report.lane.as_str(),
        report.through.as_ref().map_or_else(
            || "all registered releases in this run".into(),
            |cut| format!("through {}", text(cut))
        ),
        text(&report.retrieved_utc),
        report.ledger_anchor.anchor_id(),
        report.ledger_anchor.state_root.to_hex(),
        report.gate_eligible()
    );
    out.push_str("The certificate identifies the assessed source ledger. Canonical Claim, disclosure and model digests retain their own stated scopes; rendering adds no certification of source truth. The metric inventory keeps every selected reporting vintage.\n\n## Reading the report\n\nThe verified source inventory and visible identity ledger come first, followed by source values, cross-vintage findings, extraction limits, derived results and complete evidence-challenge packets. The final appendix reports hidden-sheet identities separately.\n\n");
    source_inventory(&mut out, report);
    ledger(&mut out, report);
    metrics(&mut out, report);
    out.push_str("\n## Issuer consistency and changes across vintages\n\nA changed value does not identify a true vintage. A silent classification means no applicable verified note in registered documents, not absence of disclosure everywhere. An incomplete disclosure registry leaves that question unassessed.\n");
    identity_history(
        &mut out,
        "AISC identity history",
        &report.issuer_consistency,
    );
    identity_history(
        &mut out,
        "Mine Stats identity history",
        &report.mine_stats_consistency.identities,
    );
    vintages(&mut out, &report.aisc_vintages);
    vintages(&mut out, &report.mine_stats_vintages);
    extraction_limits(&mut out, report);
    derived_sections(&mut out, report);
    let _ = writeln!(
        out,
        "\n## Evidence challenges and closing documents\n\n{} packets, built from the existing assessed ledger. Exact certificates, original operands, blocked uses and requested closing evidence follow; requested documents are not supplied evidence.\n",
        packets.len()
    );
    for packet in packets {
        out.push_str("\n---\n\n");
        out.push_str(&packet.to_markdown());
    }
    out.push_str("\n## Hidden-sheet appendix — excluded from public counts\n\nThese sheets were not displayed to readers. Their identities are bound in a separate ledger section, never merged into the visible population or GATE-BULL-01 denominator.\n");
    let mut hidden = FamilyCounts::<IdentityFamily>::new();
    for instance in &report.hidden_scope.instances {
        *hidden
            .entry((instance.family, instance.state.code()))
            .or_insert(0) += 1;
    }
    counts(&mut out, "Hidden AISC identities", &hidden);
    aisc_instances(&mut out, &report.hidden_scope.instances);
    for refused in &report.hidden_scope.table_refusals {
        let _ = writeln!(
            out,
            "\nHidden table refused at {}:{}: {}.\n",
            text(&refused.file),
            text(&refused.sheet),
            text(&refused.reason)
        );
    }
    for header in &report.hidden_scope.unregistered_headers {
        let _ = writeln!(
            out,
            "\nHidden unregistered header at {}:{}!{}: {}.\n",
            text(&header.file),
            text(&header.sheet),
            text(&header.cell),
            text(&header.header)
        );
    }
    let _ = writeln!(
        out,
        "\n{}\n\nFull canonical bodies and numerical certificates remain available in the unchanged JSON report.\n",
        fmine_public::NO_CLAIM
    );
    Ok(out)
}
