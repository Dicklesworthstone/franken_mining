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
            Token::Range(..) => None,
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
mod tests {
    use super::*;
    use crate::packet::packets_from_report;
    use fmine_core::hash::sha256;
    use fmine_public::sources::{DocumentRow, IssuerProfile, transcriptions_path};
    use fmine_public::transcribe::load_transcription_report;
    use fmine_public::{run_synthetic_assurance, synthetic};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture {
        root: PathBuf,
        profile: IssuerProfile,
        report: PublicAssuranceReport,
    }

    // Synthetic PDF bytes are checked, never parsed. The values are independent reader entries, as in
    // the production transcription path; none is borrowed from a real issuer's table.
    fn fixture(tokens: &[&str], context: Option<(&str, &str)>) -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "fmine-pdf-packet-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("docs")).expect("document directory");
        let mut profile = synthetic::synthetic_profile();
        let mut rows = "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        for (i, token) in tokens.iter().enumerate() {
            let file = format!("Document-{i}.pdf");
            let bytes = format!("%PDF-1.7 invented source document {i}; no parsed text");
            let sha = sha256(bytes.as_bytes()).to_hex();
            std::fs::write(root.join("docs").join(&file), bytes.as_bytes()).expect("source bytes");
            profile.documents.push(DocumentRow {
                file: file.clone(),
                dir: "docs".to_string(),
                kind: "mda".to_string(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "synthetic".to_string(),
                published_at: "2031-02-01".to_string(),
                published_at_basis: "FACT: invented date".to_string(),
                pages: 8,
                bytes: bytes.len() as u64,
                sha256: sha.clone(),
            });
            let changed = |field: &str, default: &str| {
                if i == 1
                    && let Some((name, value)) = context
                    && field == name
                {
                    value.to_string()
                } else {
                    default.to_string()
                }
            };
            for reader in ["reader_a", "reader_b"] {
                let _ = writeln!(
                    rows,
                    "\n[[entry]]\ndoc = \"{file}\"\ndoc_sha256 = \"{sha}\"\npage = 4\n\
                     table = \"Invented net changes\"\nrow = \"2030\"\ncol = \"Net change\"\n\
                     token = \"{token}\"\nquantity = \"invented_change\"\nunit = \"{}\"\n\
                     basis = \"{}\"\nperiod = \"{}\"\nreader = \"{reader}\"",
                    changed("unit", "Moz"),
                    changed("basis", "attributable"),
                    changed("period", "2030")
                );
            }
        }
        let path = transcriptions_path(&root, "synthetic");
        std::fs::create_dir_all(path.parent().expect("registry directory"))
            .expect("registry directory");
        std::fs::write(path, rows).expect("reader entries");
        let mut report = run_synthetic_assurance(&[]).expect("synthetic ledger");
        report.transcriptions =
            load_transcription_report(&profile, &root).expect("checked documents and readers");
        Fixture {
            root,
            profile,
            report,
        }
    }

    fn pdf_packets(report: &PublicAssuranceReport) -> Vec<ChallengePacket> {
        packets_from_report(report)
            .expect("valid packet set")
            .into_iter()
            .filter(|p| p.kind == PacketKind::TranscriptionConflict)
            .collect()
    }

    #[test]
    fn pdf_conflict_retains_exact_assertions_and_existing_packet_ids() {
        let f = fixture(&["5.2", "61", "61"], None);
        let original = f.report.to_json();
        let packets = pdf_packets(&f.report);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert_eq!(packet.findings[0].state, "rounding_conflict");
        let evidence = packet
            .transcription_evidence
            .as_ref()
            .expect("PDF evidence");
        assert_eq!(evidence.assertions.len(), 3);
        for a in &evidence.assertions {
            assert!(packet.to_json().contains(&js(&a.locator.to_string())));
            assert!(packet.to_json().contains(&a.doc_sha256));
            assert_eq!(a.readers, ["reader_a", "reader_b"]);
            assert!(packet.closing_documents.iter().any(|d| {
                d.locator
                    .as_ref()
                    .is_some_and(|at| at.contains(&a.locator.to_string()))
            }));
        }
        let json = packet.to_json();
        assert!(json.contains("\"exact_decimal\":{\"mantissa\":\"52\",\"scale\":\"1\"}"));
        assert!(json.contains("\"rounding_step\":{\"mantissa\":\"1\",\"scale\":\"1\"}"));
        assert!(json.contains("\"strict_assignments\":"));
        assert!(json.contains("\"convention\":\"truncate\""));
        assert!(!json.contains("\"fit\":\"proper\""));
        assert!(!json.contains("\"ledger_evidence\":"));
        assert!(json.contains("\"in_source_state_root\":false"));
        assert!(json.contains("\"human_check\":null"));
        assert!(!packet.external_use_allowed());
        assert!(packet.to_markdown().contains("Page-cited PDF assertions"));
        assert!(
            packet
                .to_markdown()
                .contains("invented synthetic documents")
        );
        assert!(
            packet
                .to_markdown()
                .contains("not an independent human review")
        );
        let mut bare = f.report.clone();
        bare.transcriptions = None;
        let old = packets_from_report(&bare).expect("existing packet population");
        let with_pdf = packets_from_report(&f.report).expect("extended population");
        for p in old {
            assert_eq!(
                with_pdf
                    .iter()
                    .find(|q| q.id == p.id)
                    .expect("unchanged id")
                    .to_json(),
                p.to_json()
            );
        }
        assert_eq!(
            f.report.to_json(),
            original,
            "no original assertion or state is rewritten"
        );
        let envelope = crate::challenge_envelope(&f.report).expect("CLI envelope");
        assert!(envelope.payload_json.contains(&json));
        assert!(
            envelope
                .rendered_markdown
                .as_ref()
                .expect("Markdown")
                .contains("Page-cited PDF assertions")
        );
        assert_ne!(
            envelope.payload_json,
            crate::challenge_envelope(&bare)
                .expect("baseline envelope")
                .payload_json
        );
        assert_eq!(
            f.report.ledger_anchor.state_root,
            bare.ledger_anchor.state_root
        );
    }

    #[test]
    fn pdf_truncation_ranges_and_refused_discovery_do_not_become_conflicts() {
        // Closed intervals do not meet, but truncation of the first print reconciles them.
        let f = fixture(&["3.6", "3.68"], None);
        let t = f.report.transcriptions.as_ref().expect("loaded");
        let d = t
            .tie
            .as_ref()
            .expect("tie")
            .as_ref()
            .expect("supported")
            .discovery
            .as_ref()
            .expect("discovery");
        assert_eq!(d.closed.fit, Fit::Empty);
        assert!(d.conflicts.is_empty());
        assert!(pdf_packets(&f.report).is_empty());
        assert!(pdf_packets(&fixture(&["20 - 30", "40 - 50"], None).report).is_empty());
        assert!(pdf_packets(&fixture(&["5.2"], None).report).is_empty());
        let f = fixture(&["1", "10", "20", "30", "40"], None);
        assert!(matches!(
            f.report.transcriptions.as_ref().expect("loaded").tie,
            Some(Err(_))
        ));
        assert!(
            pdf_packets(&f.report).is_empty(),
            "a refused global discovery is not selectively promoted"
        );
    }

    #[test]
    fn pdf_context_mismatch_never_claims_a_numeric_discrepancy() {
        for context in [
            ("basis", "consolidated"),
            ("unit", "Mlb"),
            ("period", "2029"),
        ] {
            let packets = pdf_packets(&fixture(&["5.2", "61"], Some(context)).report);
            assert_eq!(packets.len(), 1);
            assert_eq!(packets[0].findings[0].state, "quantity_context_mismatch");
            assert!(packets[0].to_json().contains("\"rounding\":null"));
            assert!(
                packets[0]
                    .to_json()
                    .contains("\"numeric_comparability\":\"not_established\"")
            );
            assert!(packets[0].to_json().contains(&js(context.1)));
        }
        let packets =
            pdf_packets(&fixture(&["5.2", "5.2"], Some(("basis", "consolidated"))).report);
        assert_eq!(
            packets[0].findings[0].state, "quantity_context_mismatch",
            "matching digits do not establish comparable quantities"
        );
    }

    #[test]
    fn pdf_packets_refuse_forged_comparisons_tokens_hashes_and_readers() {
        let f = fixture(&["5.2", "61"], None);
        for (mutation, code) in [
            ("omit", "FMINE-REF-208"),
            ("locator", "FMINE-REF-208"),
            ("token", "FMINE-REF-208"),
            ("hash", "FMINE-REF-505"),
            ("reader", "FMINE-REF-202"),
            ("duplicate", "FMINE-REF-208"),
            ("missing_document", "FMINE-REF-506"),
        ] {
            let mut report = f.report.clone();
            let t = report.transcriptions.as_mut().expect("loaded");
            match mutation {
                "omit" => t
                    .tie
                    .as_mut()
                    .expect("tie")
                    .as_mut()
                    .expect("supported")
                    .conflicts
                    .clear(),
                "locator" => {
                    t.tie
                        .as_mut()
                        .expect("tie")
                        .as_mut()
                        .expect("supported")
                        .conflicts[0]
                        .1[0]
                        .page += 1
                }
                "token" => t.agreement.accepted[0].token = "5.3".to_string(),
                "hash" => t.agreement.accepted[0].doc_sha256 = "0".repeat(64),
                "reader" => t.agreement.accepted[0].readers = vec!["reader_a".to_string(); 2],
                "duplicate" => t.agreement.accepted.push(t.agreement.accepted[0].clone()),
                "missing_document" => t.documents.clear(),
                _ => unreachable!(),
            }
            assert_eq!(
                packets_from_report(&report).expect_err(mutation).code,
                code,
                "{mutation}"
            );
        }
    }

    #[test]
    fn pdf_packet_ids_bind_values_metadata_and_are_order_independent() {
        let f = fixture(&["5.2", "61", "61"], None);
        let original = pdf_packets(&f.report).remove(0);
        let mut reordered = f.report.clone();
        let t = reordered.transcriptions.as_mut().expect("loaded");
        t.documents.reverse();
        t.agreement.accepted.reverse();
        for a in &mut t.agreement.accepted {
            a.readers.reverse();
        }
        if let Some(Ok(outcome)) = &mut t.tie {
            outcome.conflicts.reverse();
            for (_, at) in &mut outcome.conflicts {
                at.reverse();
            }
        }
        let reordered = pdf_packets(&reordered).remove(0);
        assert_eq!(reordered.to_json(), original.to_json());
        assert_eq!(reordered.to_markdown(), original.to_markdown());
        assert_ne!(
            pdf_packets(&fixture(&["5.3", "61", "61"], None).report)[0].id,
            original.id
        );
        let mut changed = f.report.clone();
        changed.transcriptions.as_mut().expect("loaded").documents[0].published_at_basis =
            "INFERENCE: revised date basis".to_string();
        assert_ne!(pdf_packets(&changed)[0].id, original.id);
    }

    #[test]
    fn pdf_source_loader_refuses_altered_missing_and_disagreed_inputs() {
        let f = fixture(&["5.2", "61"], None);
        let document = f.root.join("docs/Document-0.pdf");
        std::fs::write(&document, b"%PDF-1.7 changed source").expect("alter synthetic bytes");
        assert_eq!(
            load_transcription_report(&f.profile, &f.root)
                .expect_err("altered")
                .code,
            "FMINE-REF-505"
        );
        std::fs::rename(&document, f.root.join("docs/Document-0-retained.pdf"))
            .expect("preserve bytes at different path");
        assert_eq!(
            load_transcription_report(&f.profile, &f.root)
                .expect_err("missing")
                .code,
            "FMINE-REF-506"
        );
        let f = fixture(&["5.2", "61"], None);
        let path = transcriptions_path(&f.root, "synthetic");
        let rows = std::fs::read_to_string(&path).expect("entries");
        std::fs::write(path, rows.replacen("token = \"5.2\"", "token = \"5.3\"", 1))
            .expect("conflicting reader");
        assert_eq!(
            load_transcription_report(&f.profile, &f.root)
                .expect_err("reader conflict")
                .code,
            "FMINE-REF-202"
        );
    }

    #[test]
    #[ignore = "requires registered Barrick workbooks, PDFs and a local two-reader transcription registry"]
    fn barrick_pdf_packets_cover_registered_conflicts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let report = fmine_public::run_barrick_m0_assurance(&root).expect("registered sources");
        let t = report
            .transcriptions
            .as_ref()
            .expect("local PDF transcriptions are required");
        let conflicts = &t
            .tie
            .as_ref()
            .expect("shared quantities")
            .as_ref()
            .expect("supported discovery")
            .conflicts;
        assert!(
            !conflicts.is_empty(),
            "the independently read PDF conflict must be present"
        );
        let packets = pdf_packets(&report);
        for (quantity, locators) in conflicts {
            let matches: Vec<_> = packets.iter().filter(|p| p.subject == *quantity).collect();
            assert_eq!(matches.len(), 1, "{quantity} covered exactly once");
            let evidence = matches[0]
                .transcription_evidence
                .as_ref()
                .expect("source evidence");
            for at in locators {
                let assertion = evidence
                    .assertions
                    .iter()
                    .find(|a| a.locator == *at)
                    .expect("every cited assertion");
                let doc = t
                    .documents
                    .iter()
                    .find(|d| d.file == at.doc)
                    .expect("checked document");
                assert_eq!(assertion.doc_sha256, doc.sha256);
                assert_eq!(
                    parse_token(&assertion.token).expect("exact source token"),
                    assertion.value
                );
            }
            assert!(!matches[0].external_use_allowed());
            matches[0]
                .validate()
                .expect("complete internal review packet");
        }
        let envelope = crate::challenge_envelope(&report).expect("real CLI envelope");
        assert!(
            envelope
                .payload_json
                .contains("\"kind\":\"transcription_conflict\"")
        );
        assert!(
            envelope
                .rendered_markdown
                .as_ref()
                .expect("Markdown")
                .contains("Page-cited PDF assertions")
        );
    }
}
