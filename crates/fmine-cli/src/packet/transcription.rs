//! Report-only PDF evidence. The assurance loader verifies document bytes and reader entries; this
//! projection checks the retained records and recomputes the comparison, without pretending to parse PDFs.

use super::{
    ChallengePacket, ClosingDocument, Finding, GroupKey, PacketKind, js, md, packet_for, strings,
    unusable,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::discover::{
    Assignment, Fit, Printed, TieDiscovery, discover_tie_conventions,
};
use fmine_numerics::exact_feas::{Bound, Convention, Rat};
use fmine_public::PublicAssuranceReport;
use fmine_public::transcribe::{CheckedDocument, TieOutcome, TranscriptionReport};
use fmine_registry::transcriptions::{Accepted, Agreement, Decimal, Token, parse_token};

const BINDING: &str = "The transcription loader checks document bytes. These retained document hashes and accepted rows are bound by the packet, separately from the workbook ledger root. Matching reader roles are not an independent human review of this packet.";

#[derive(Debug, Clone)]
pub(super) struct Evidence {
    registry: String,
    accepted_digest: String,
    documents: Vec<CheckedDocument>,
    assertions: Vec<Accepted>,
}

impl Evidence {
    pub(super) fn validate(&self) -> Result<(), Refusal> {
        if self.registry.is_empty() || !digest(&self.accepted_digest) || self.assertions.len() < 2 {
            return Err(unusable(
                "PDF packet needs its registry, accepted-row digest and assertions",
            ));
        }
        validate_assertions(&self.documents, &self.assertions)
    }

    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.pdf_packet_evidence.v1\",\"registry\":{},\"accepted_digest\":{},\
             \"source_binding\":{},\"documents\":[{}],\"assertions\":[{}]}}",
            js(&self.registry),
            js(&self.accepted_digest),
            js(BINDING),
            self.documents
                .iter()
                .map(document_json)
                .collect::<Vec<_>>()
                .join(","),
            self.assertions
                .iter()
                .map(assertion_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    pub(super) fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Page-cited PDF assertions\n\n{}\n\nRegistry: {}\n\nAccepted-row digest: {}\n\n\
             | Locator | Raw token | Unit | Basis | Period | SHA-256 |\n\
             | --- | --- | --- | --- | --- | --- |\n",
            BINDING,
            md(&self.registry),
            self.accepted_digest
        );
        for a in &self.assertions {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} |",
                md(&a.locator.to_string()),
                md(&a.token),
                md(&a.unit),
                md(&a.basis),
                md(&a.period),
                a.doc_sha256
            );
        }
        for d in &self.documents {
            let _ = writeln!(
                out,
                "\n- {}: publication date {}; date basis: {}.",
                md(&d.file),
                md(&d.published_at),
                md(&d.published_at_basis)
            );
        }
        out
    }
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_assertions(
    documents: &[CheckedDocument],
    accepted: &[Accepted],
) -> Result<(), Refusal> {
    let mut docs = BTreeMap::new();
    for d in documents {
        if d.file.is_empty() || !digest(&d.sha256) || docs.insert(&d.file, d).is_some() {
            return Err(unusable(
                "PDF checked-document identities are missing or duplicated",
            ));
        }
    }
    let mut locators = BTreeSet::new();
    let mut quantities = BTreeSet::new();
    for a in accepted {
        let Some(document) = docs.get(&a.locator.doc) else {
            return Err(Refusal::new(
                RefusalCode::InputMissingOrUnreadable506,
                format!("no checked PDF document for {}", a.locator),
            ));
        };
        if a.doc_sha256 != document.sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "PDF transcription and checked document hashes differ at {}",
                    a.locator
                ),
            ));
        }
        if a.locator.page == 0
            || [
                &a.locator.table,
                &a.locator.row,
                &a.locator.col,
                &a.quantity,
                &a.unit,
                &a.basis,
                &a.period,
            ]
            .iter()
            .any(|s| s.trim().is_empty())
            || !locators.insert(&a.locator)
            || !quantities.insert((&a.locator.doc, &a.quantity))
        {
            return Err(unusable(format!(
                "incomplete or duplicate PDF assertion at {}",
                a.locator
            )));
        }
        let readers: BTreeSet<_> = a.readers.iter().collect();
        if readers.len() < 2
            || readers.len() != a.readers.len()
            || readers.iter().any(|r| r.is_empty())
        {
            return Err(Refusal::new(
                RefusalCode::NotObservable202,
                format!(
                    "PDF assertion needs distinct agreeing reader roles at {}",
                    a.locator
                ),
            ));
        }
        if parse_token(&a.token).ok() != Some(a.value) {
            return Err(unusable(format!(
                "PDF token and exact value differ at {}",
                a.locator
            )));
        }
    }
    Ok(())
}

fn decimal(d: Decimal) -> Result<(Rat, Rat), ()> {
    let den = 10_i128.checked_pow(d.scale).ok_or(())?;
    Ok((
        Rat::new(d.mantissa, den).map_err(|_| ())?,
        Rat::new(1, den).map_err(|_| ())?,
    ))
}

fn points(accepted: &[Accepted]) -> Result<Vec<Printed>, ()> {
    accepted
        .iter()
        .filter_map(|a| match a.value {
            Token::Point(d) => Some(decimal(d).map(|(printed, step)| Printed {
                document: a.locator.doc.clone(),
                quantity: a.quantity.clone(),
                printed,
                step,
            })),
            Token::Range(..) | Token::AtMost(_) | Token::AtLeast(_) => None,
        })
        .collect()
}

// Match the existing report's population, including its global document limit and re-run after conflicts.
// A refusal remains unknown; it is never upgraded by running a smaller, selectively chosen comparison.
fn recompute(accepted: &[Accepted]) -> Option<Result<TieOutcome, ()>> {
    let printed = match points(accepted) {
        Ok(p) => p,
        Err(()) => return Some(Err(())),
    };
    let mut documents: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for p in &printed {
        documents
            .entry(&p.quantity)
            .or_default()
            .insert(&p.document);
    }
    let printed: Vec<_> = printed
        .iter()
        .filter(|p| documents[p.quantity.as_str()].len() > 1)
        .cloned()
        .collect();
    if printed.is_empty() {
        return None;
    }
    Some((|| {
        let first = discover_tie_conventions(&printed).map_err(|_| ())?;
        let conflicts: Vec<_> = first
            .conflicts
            .iter()
            .map(|q| {
                let locators = accepted
                    .iter()
                    .filter(|a| &a.quantity == q && matches!(a.value, Token::Point(_)))
                    .map(|a| a.locator.clone())
                    .collect();
                (q.clone(), locators)
            })
            .collect();
        let discovery = if conflicts.is_empty() {
            Some(first)
        } else {
            let rest: Vec<_> = printed
                .into_iter()
                .filter(|p| !first.conflicts.contains(&p.quantity))
                .collect();
            let docs: BTreeSet<_> = rest.iter().map(|p| &p.document).collect();
            if docs.len() > 1 {
                Some(discover_tie_conventions(&rest).map_err(|_| ())?)
            } else {
                None
            }
        };
        Ok(TieOutcome {
            conflicts,
            discovery,
        })
    })())
}

fn canonical_outcome(mut outcome: TieOutcome) -> TieOutcome {
    for (_, at) in &mut outcome.conflicts {
        at.sort();
    }
    outcome.conflicts.sort();
    outcome
}

fn checked_conflicts(
    t: &TranscriptionReport,
    accepted: &[Accepted],
) -> Result<BTreeSet<String>, Refusal> {
    if !t.agreement.is_consistent() {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            "PDF reader disagreements remain unresolved",
        ));
    }
    validate_assertions(&t.documents, accepted)?;
    let entries = accepted
        .iter()
        .try_fold(t.agreement.single_reader.len(), |n, a| {
            n.checked_add(a.readers.len())
        });
    if entries != Some(t.entries) {
        return Err(unusable(
            "PDF accepted and pending rows do not account for the registered entry count",
        ));
    }
    match (recompute(accepted), &t.tie) {
        (None, None) | (Some(Err(())), Some(Err(_))) => Ok(BTreeSet::new()),
        (Some(Ok(actual)), Some(Ok(supplied)))
            if canonical_outcome(actual.clone()) == canonical_outcome(supplied.clone()) =>
        {
            Ok(actual
                .conflicts
                .into_iter()
                .map(|(quantity, _)| quantity)
                .collect())
        }
        _ => Err(unusable(
            "PDF tie findings differ from recomputed accepted transcription assertions",
        )),
    }
}

fn document_json(d: &CheckedDocument) -> String {
    format!(
        "{{\"file\":{},\"kind\":{},\"sha256\":{},\"published_at\":{},\"published_at_basis\":{}}}",
        js(&d.file),
        js(&d.kind),
        js(&d.sha256),
        js(&d.published_at),
        js(&d.published_at_basis)
    )
}

fn decimal_json(d: Decimal) -> String {
    format!(
        "{{\"mantissa\":{},\"scale\":{}}}",
        js(&d.mantissa.to_string()),
        js(&d.scale.to_string())
    )
}

fn assertion_json(a: &Accepted) -> String {
    let value = match a.value {
        Token::Point(d) => format!(
            "{{\"kind\":\"point\",\"exact_decimal\":{},\"rounding_step\":{}}}",
            decimal_json(d),
            decimal_json(Decimal {
                mantissa: 1,
                scale: d.scale
            })
        ),
        Token::Range(lo, hi) => format!(
            "{{\"kind\":\"range\",\"lo\":{},\"hi\":{},\"comparison\":\"range_not_a_point\"}}",
            decimal_json(lo),
            decimal_json(hi)
        ),
        Token::AtMost(d) => format!(
            "{{\"kind\":\"at_most\",\"upper\":{},\"comparison\":\"inclusive_bound_not_a_point\"}}",
            decimal_json(d)
        ),
        Token::AtLeast(d) => format!(
            "{{\"kind\":\"at_least\",\"lower\":{},\"comparison\":\"inclusive_bound_not_a_point\"}}",
            decimal_json(d)
        ),
    };
    format!(
        "{{\"at\":{},\"sha256\":{},\"page\":{},\"table\":{},\"row\":{},\"column\":{},\"raw_token\":{},\
         \"value\":{},\"quantity\":{},\"unit\":{},\"basis\":{},\"period\":{},\"reader_roles\":{},\
         \"claim\":null,\"evidence_class\":\"transcribed\"}}",
        js(&a.locator.to_string()),
        js(&a.doc_sha256),
        js(&a.locator.page.to_string()),
        js(&a.locator.table),
        js(&a.locator.row),
        js(&a.locator.col),
        js(&a.token),
        value,
        js(&a.quantity),
        js(&a.unit),
        js(&a.basis),
        js(&a.period),
        strings(a.readers.iter().map(String::as_str))
    )
}

const fn convention(c: Convention) -> &'static str {
    match c {
        Convention::Closed => "closed",
        Convention::HalfUp => "half_up",
        Convention::HalfDown => "half_down",
        Convention::HalfEven => "half_even",
        Convention::Truncate => "truncate",
    }
}

fn bound_json(b: Bound) -> String {
    let end = |e: Option<fmine_numerics::exact_feas::End>| {
        e.map_or_else(
            || "null".to_string(),
            |e| {
                format!(
                    "{{\"numerator\":{},\"denominator\":{},\"strict\":{}}}",
                    js(&e.at.num().to_string()),
                    js(&e.at.den().to_string()),
                    e.strict
                )
            },
        )
    };
    format!("{{\"lo\":{},\"hi\":{}}}", end(b.lo), end(b.hi))
}

fn assignment_json(a: &Assignment) -> String {
    let conventions: Vec<_> = a
        .conventions
        .iter()
        .map(|(doc, c)| {
            format!(
                "{{\"document\":{},\"convention\":{}}}",
                js(doc),
                js(convention(*c))
            )
        })
        .collect();
    let ranges: Vec<_> = a
        .ranges
        .iter()
        .map(|(q, range)| format!("{{\"quantity\":{},\"meet\":{}}}", js(q), bound_json(*range)))
        .collect();
    let fit = match a.fit {
        Fit::Empty => "empty",
        Fit::Points => "points",
        Fit::Proper => "proper",
    };
    format!(
        "{{\"conventions\":[{}],\"ranges\":[{}],\"fit\":{}}}",
        conventions.join(","),
        ranges.join(","),
        js(fit)
    )
}

fn rounding_json(d: &TieDiscovery, assertions: &[Accepted]) -> String {
    format!(
        "{{\"rule\":\"pdf.same_quantity_rounding\",\"rules_version\":\"1\",\"inputs\":{},\
         \"closed\":{},\"strict_assignments\":[{}]}}",
        strings(
            assertions
                .iter()
                .filter(|a| matches!(a.value, Token::Point(_)))
                .map(|a| a.locator.to_string())
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str)
        ),
        assignment_json(&d.closed),
        d.assignments
            .iter()
            .map(assignment_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn add_packets(
    groups: &mut BTreeMap<GroupKey, ChallengePacket>,
    report: &PublicAssuranceReport,
) -> Result<(), Refusal> {
    let Some(t) = &report.transcriptions else {
        return Ok(());
    };
    let mut accepted = t.agreement.accepted.clone();
    accepted.sort_by(|a, b| a.locator.cmp(&b.locator));
    for a in &mut accepted {
        a.readers.sort();
    }
    let conflicts = checked_conflicts(t, &accepted)?;
    let digest = Agreement {
        accepted: accepted.clone(),
        ..Agreement::default()
    }
    .digest();
    let mut quantities: BTreeMap<&str, Vec<Accepted>> = BTreeMap::new();
    for a in &accepted {
        quantities.entry(&a.quantity).or_default().push(a.clone());
    }
    for (quantity, assertions) in quantities {
        if assertions.len() < 2 {
            continue;
        }
        let first = &assertions[0];
        let mismatch = assertions
            .iter()
            .any(|a| (&a.unit, &a.basis, &a.period) != (&first.unit, &first.basis, &first.period));
        if !mismatch && !conflicts.contains(quantity) {
            continue;
        }
        let (state, rounding) = if mismatch {
            ("quantity_context_mismatch", "null".to_string())
        } else {
            let printed = points(&assertions)
                .map_err(|()| unusable("PDF point cannot be represented exactly"))?;
            let comparison = discover_tie_conventions(&printed)
                .map_err(|e| unusable(format!("PDF conflict cannot be replayed: {e:?}")))?;
            if !comparison.conflicts.iter().any(|q| q == quantity) {
                return Err(unusable(
                    "PDF quantity no longer has the reported rounding conflict",
                ));
            }
            ("rounding_conflict", rounding_json(&comparison, &assertions))
        };
        let at = assertions
            .iter()
            .map(|a| a.locator.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        let documents: BTreeSet<_> = assertions.iter().map(|a| &a.locator.doc).collect();
        let mut checked: Vec<_> = t
            .documents
            .iter()
            .filter(|d| documents.contains(&d.file))
            .cloned()
            .collect();
        checked.sort_by(|a, b| a.file.cmp(&b.file));
        let packet = packet_for(
            groups,
            report,
            GroupKey {
                origin: "pdf_transcription",
                subject: quantity.to_string(),
                family: "pdf_transcription".to_string(),
                kind: PacketKind::TranscriptionConflict,
                discriminator: quantity.to_string(),
            },
            "registered quantity context; all supported per-document rounding conventions",
        );
        packet.findings.push(Finding {
            period: if assertions.iter().all(|a| a.period == first.period) { first.period.clone() } else { "incompatible registered periods".to_string() },
            at: at.clone(), state: state.to_string(),
            evidence_json: format!(
                "{{\"schema\":\"fmine.pdf_conflict.v1\",\"quantity\":{},\"finding\":{},\
                 \"report_only\":true,\"in_source_state_root\":false,\"disclosure_status\":\"not_assessed\",\
                 \"numeric_comparability\":{},\"rounding\":{}}}",
                js(quantity), js(state), js(if mismatch { "not_established" } else { "under_registered_quantity_context" }), rounding
            ),
        });
        packet.closing_documents.insert(ClosingDocument {
            document: "independent page reread and documented quantity reconciliation".to_string(),
            locator: Some(at),
            would_show: if mismatch {
                "Confirm the printed unit, ownership or metric basis and period at every cited page; correct the quantity mapping or provide an explicit bridge before comparing values."
            } else {
                "Re-read every cited token and label, confirm that the assertions describe the same quantity, and provide the source-supported correction or reconciliation. A published explanation is required before assigning disclosure status."
            }.to_string(),
        });
        packet.transcription_evidence = Some(Evidence {
            registry: t.registry.clone(),
            accepted_digest: digest.clone(),
            documents: checked,
            assertions,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../xtask/testdata/packet_transcription_controls.rs"]
mod tests;
