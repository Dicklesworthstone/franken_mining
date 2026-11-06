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
    let packets = pdf_packets(&fixture(&["5.2", "5.2"], Some(("basis", "consolidated"))).report);
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
