//! Transcribed values from registered PDF documents (bd-plx1), and the rounding-convention discovery they feed
//! (bd-4wvl.4 part 2).
//!
//! When `<workspace>/registries/transcriptions/<issuer>.toml` exists, a run checks the bytes of every document
//! it cites against the sources registry and refuses naming each missing (`FMINE-REF-506`, exit 5) or altered
//! (`FMINE-REF-505`, exit 8) file. It then applies the two-reader check and refuses (`FMINE-REF-202`, not
//! observable; exit 3), naming the locators, if any readers disagree; a locator only
//! one reader has entered is pending and reported. Accepted values that two or more documents print under one
//! quantity key form a tie, and [`discover_tie_conventions`] finds which rounding convention each document
//! uses. A quantity no convention reconciles is reported as a conflict with its locators, and the discovery is
//! re-run without it. Everything here is report-only: never a state, not in the state root. The file is absent
//! in CI and in a fresh clone (git-ignored pending OPEN-R7-1), and then the section is `null`.

use std::fmt::Write as _;
use std::path::Path;

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::exact_feas::discover::{
    Fit, Printed, TieDiscovery, TieFinding, TieRefusal, discover_tie_conventions,
};
use fmine_numerics::exact_feas::{Convention, Rat};
use fmine_registry::transcriptions::{
    Accepted, Agreement, Decimal, Locator, Token, agree, load_transcriptions,
};

use crate::js;
use crate::sources::{DocumentRow, IssuerProfile, transcriptions_path};

/// One cited document whose bytes matched the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedDocument {
    pub file: String,
    pub kind: String,
    pub published_at: String,
    pub published_at_basis: String,
    pub sha256: String,
}

/// The tie discovery over accepted values, with the irreconcilable quantities taken out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TieOutcome {
    /// Quantities no convention reconciles, each with the locators that print it.
    pub conflicts: Vec<(String, Vec<Locator>)>,
    /// The discovery without the conflicts, if two or more documents still share a quantity.
    pub discovery: Option<TieDiscovery>,
}

/// What a run read from its transcription registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionReport {
    /// The registry file, relative to the workspace root.
    pub registry: String,
    /// Cited documents, in registry order.
    pub documents: Vec<CheckedDocument>,
    pub entries: usize,
    pub agreement: Agreement,
    /// `None` when no quantity is printed by two documents; `Err` names why the discovery was refused.
    pub tie: Option<Result<TieOutcome, String>>,
}

/// Checks the cited documents' bytes, refusing with every missing or altered file named.
fn check_documents(cited: &[&DocumentRow], root: &Path) -> Result<Vec<CheckedDocument>, Refusal> {
    let (mut missing, mut altered, mut checked) = (Vec::new(), Vec::new(), Vec::new());
    for d in cited {
        let bytes = match std::fs::read(root.join(&d.dir).join(&d.file)) {
            Ok(b) => b,
            Err(e) => {
                missing.push(format!("{} ({e})", d.file));
                continue;
            }
        };
        let digest = sha256(&bytes).to_hex();
        if bytes.len() as u64 != d.bytes || digest != d.sha256 {
            altered.push(format!(
                "{} (registered {} bytes sha256 {}, found {} bytes sha256 {digest})",
                d.file,
                d.bytes,
                d.sha256,
                bytes.len()
            ));
            continue;
        }
        checked.push(CheckedDocument {
            file: d.file.clone(),
            kind: d.kind.clone(),
            published_at: d.published_at.clone(),
            published_at_basis: d.published_at_basis.clone(),
            sha256: d.sha256.clone(),
        });
    }
    if missing.is_empty() && altered.is_empty() {
        return Ok(checked);
    }
    let mut detail = format!(
        "{} of {} cited documents unusable",
        missing.len() + altered.len(),
        cited.len()
    );
    for (label, list) in [("missing", &missing), ("digest mismatch", &altered)] {
        if !list.is_empty() {
            let _ = write!(detail, "; {label}: {}", list.join(", "));
        }
    }
    Err(crate::files_refusal(&missing, &[], detail))
}

/// An exact rational and its rounding step from a printed decimal.
fn printed_value(d: Decimal) -> Result<(Rat, Rat), String> {
    let den = 10_i128
        .checked_pow(d.scale)
        .ok_or_else(|| "a scale too large".to_string())?;
    let value = Rat::new(d.mantissa, den).map_err(|e| format!("{e:?}"))?;
    let step = Rat::new(1, den).map_err(|e| format!("{e:?}"))?;
    Ok((value, step))
}

fn tie_refusal(e: &TieRefusal) -> String {
    match e {
        TieRefusal::NonPositiveStep(d, q) => format!("non-positive step at {d}/{q}"),
        TieRefusal::Duplicate(d, q) => format!("{d} prints {q} twice"),
        TieRefusal::DocumentCount(n) => format!("{n} documents share quantities (2 to 4 allowed)"),
        TieRefusal::Interval(d, q, e) => format!("interval at {d}/{q}: {e:?}"),
    }
}

/// The tie over accepted point values that two or more documents print under one quantity key.
fn tie_outcome(accepted: &[Accepted]) -> Option<Result<TieOutcome, String>> {
    let points: Vec<(&Accepted, Decimal)> = accepted
        .iter()
        .filter_map(|a| match a.value {
            Token::Point(d) => Some((a, d)),
            Token::Range(..) => None,
        })
        .collect();
    let shared = |q: &str| {
        let mut docs: Vec<&str> = points
            .iter()
            .filter(|(a, _)| a.quantity == q)
            .map(|(a, _)| a.locator.doc.as_str())
            .collect();
        docs.sort_unstable();
        docs.dedup();
        docs.len() > 1
    };
    let tied: Vec<&(&Accepted, Decimal)> =
        points.iter().filter(|(a, _)| shared(&a.quantity)).collect();
    if tied.is_empty() {
        return None;
    }
    let mut printed = Vec::with_capacity(tied.len());
    for (a, d) in &tied {
        let (value, step) = match printed_value(*d) {
            Ok(v) => v,
            Err(e) => return Some(Err(format!("{}: {e}", a.locator))),
        };
        printed.push(Printed {
            document: a.locator.doc.clone(),
            quantity: a.quantity.clone(),
            printed: value,
            step,
        });
    }
    let first = match discover_tie_conventions(&printed) {
        Ok(t) => t,
        Err(e) => return Some(Err(tie_refusal(&e))),
    };
    let conflicts: Vec<(String, Vec<Locator>)> = first
        .conflicts
        .iter()
        .map(|q| {
            let at = tied
                .iter()
                .filter(|(a, _)| &a.quantity == q)
                .map(|(a, _)| a.locator.clone())
                .collect();
            (q.clone(), at)
        })
        .collect();
    if conflicts.is_empty() {
        return Some(Ok(TieOutcome {
            conflicts,
            discovery: Some(first),
        }));
    }
    let rest: Vec<Printed> = printed
        .into_iter()
        .filter(|p| !first.conflicts.contains(&p.quantity))
        .collect();
    let mut docs: Vec<&str> = rest.iter().map(|p| p.document.as_str()).collect();
    docs.sort_unstable();
    docs.dedup();
    let discovery = if docs.len() > 1 {
        match discover_tie_conventions(&rest) {
            Ok(t) => Some(t),
            Err(e) => return Some(Err(tie_refusal(&e))),
        }
    } else {
        None
    };
    Some(Ok(TieOutcome {
        conflicts,
        discovery,
    }))
}

/// Reads `<root>/registries/transcriptions/<issuer>.toml` if it exists (`Ok(None)` if not), checks the cited
/// documents and the two-reader agreement, and runs the tie discovery.
pub fn load_transcription_report(
    profile: &IssuerProfile,
    root: &Path,
) -> Result<Option<TranscriptionReport>, Refusal> {
    let path = transcriptions_path(root, &profile.issuer);
    if !path.is_file() {
        return Ok(None);
    }
    let registry = format!("registries/transcriptions/{}.toml", profile.issuer);
    let reg = load_transcriptions(&path, &profile.issuer, &profile.documents)
        .map_err(|e| crate::input_refusal(format!("transcription registry {registry}: {e}")))?;
    let cited: Vec<&DocumentRow> = profile
        .documents
        .iter()
        .filter(|d| reg.entries.iter().any(|e| e.locator.doc == d.file))
        .collect();
    let documents = check_documents(&cited, root)?;
    let agreement = agree(&reg);
    if !agreement.is_consistent() {
        let mut detail = format!("transcription registry {registry}: readers disagree");
        for d in &agreement.disagreements {
            let _ = write!(
                detail,
                "; at {} ({}: {})",
                d.locator,
                d.readers.join(", "),
                d.fields.join(", ")
            );
        }
        for (doc, quantity, at) in &agreement.quantity_clashes {
            let at: Vec<String> = at.iter().map(ToString::to_string).collect();
            let _ = write!(
                detail,
                "; {doc} quantity {quantity} at two locators ({})",
                at.join(", ")
            );
        }
        // Conflicted evidence is a 2xx refusal (§30.1): until the readers agree, the printed value is not
        // observable from the transcription.
        return Err(Refusal::new(RefusalCode::NotObservable202, detail));
    }
    let tie = tie_outcome(&agreement.accepted);
    Ok(Some(TranscriptionReport {
        registry,
        documents,
        entries: reg.entries.len(),
        agreement,
        tie,
    }))
}

const fn convention_name(c: Convention) -> &'static str {
    match c {
        Convention::Closed => "closed",
        Convention::HalfUp => "half_up",
        Convention::HalfDown => "half_down",
        Convention::HalfEven => "half_even",
        Convention::Truncate => "truncate",
    }
}

const fn fit_name(f: Fit) -> &'static str {
    match f {
        Fit::Empty => "empty",
        Fit::Points => "points",
        Fit::Proper => "proper",
    }
}

fn strings(items: impl IntoIterator<Item = String>) -> String {
    items
        .into_iter()
        .map(|s| js(&s))
        .collect::<Vec<_>>()
        .join(",")
}

fn discovery_json(t: &TieDiscovery) -> String {
    let count = |f: Fit| t.assignments.iter().filter(|a| a.fit == f).count();
    let findings: Vec<String> = t
        .findings
        .iter()
        .map(|(doc, finding)| {
            let (name, conventions) = match finding {
                TieFinding::Identified(c) => ("identified", vec![*c]),
                TieFinding::Ambiguous(cs) => ("ambiguous", cs.clone()),
                TieFinding::NoProperFit => ("no_proper_fit", Vec::new()),
            };
            format!(
                "{{\"document\":{},\"finding\":{},\"conventions\":[{}],\"reason_code\":{}}}",
                js(doc),
                js(name),
                strings(
                    conventions
                        .into_iter()
                        .map(|c| convention_name(c).to_string())
                ),
                finding.reason_code().map_or_else(|| "null".to_string(), js)
            )
        })
        .collect();
    format!(
        "{{\"documents\":[{}],\"quantities\":[{}],\"assignments\":{},\"fits\":{{\"proper\":{},\"points\":{},\
         \"empty\":{}}},\"closed_fit\":{},\"findings\":[{}]}}",
        strings(t.closed.conventions.iter().map(|(d, _)| d.clone())),
        strings(t.closed.ranges.iter().map(|(q, _)| q.clone())),
        t.assignments.len(),
        count(Fit::Proper),
        count(Fit::Points),
        count(Fit::Empty),
        js(fit_name(t.closed.fit)),
        findings.join(",")
    )
}

/// The `transcriptions` section: citations, counts, digests and conventions; never a transcribed value.
#[must_use]
pub fn transcription_json(report: Option<&TranscriptionReport>) -> String {
    let Some(r) = report else {
        return "\"transcriptions\":null,".to_string();
    };
    let documents: Vec<String> = r
        .documents
        .iter()
        .map(|d| {
            format!(
                "{{\"file\":{},\"kind\":{},\"published_at\":{},\"published_at_basis\":{},\"sha256\":{}}}",
                js(&d.file),
                js(&d.kind),
                js(&d.published_at),
                js(&d.published_at_basis),
                js(&d.sha256)
            )
        })
        .collect();
    let tie = match &r.tie {
        None => "null".to_string(),
        Some(Err(e)) => format!("{{\"refused\":{}}}", js(e)),
        Some(Ok(t)) => {
            let conflicts: Vec<String> = t
                .conflicts
                .iter()
                .map(|(q, at)| {
                    format!(
                        "{{\"quantity\":{},\"at\":[{}]}}",
                        js(q),
                        strings(at.iter().map(ToString::to_string))
                    )
                })
                .collect();
            format!(
                "{{\"conflicts\":[{}],\"discovery\":{}}}",
                conflicts.join(","),
                t.discovery
                    .as_ref()
                    .map_or_else(|| "null".to_string(), discovery_json)
            )
        }
    };
    format!(
        "\"transcriptions\":{{\"report_only\":\"values transcribed from registered PDF documents by two readers; \
         never a state, not in the state root\",\"registry\":{},\"documents\":[{}],\"entries\":{},\"accepted\":{},\
         \"accepted_digest\":{},\"single_reader\":[{}],\"tie\":{}}},",
        js(&r.registry),
        documents.join(","),
        r.entries,
        r.agreement.accepted.len(),
        js(&r.agreement.digest()),
        strings(r.agreement.single_reader.iter().map(|(l, _)| l.to_string())),
        tie
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::load_issuer_profile;
    use crate::{run_public_assurance, synthetic};

    const DECK: &[u8] = b"%PDF-1.7 synthetic deck, never parsed";
    const MDA: &[u8] = b"%PDF-1.7 synthetic MD&A, never parsed";

    /// A synthetic workspace: the synthetic sources registry with two registered documents, their bytes, and a
    /// transcription registry of `rows`.
    fn workspace(tag: &str, rows: &[String]) -> std::path::PathBuf {
        let ws =
            std::env::temp_dir().join(format!("fmine-transcribe-{tag}-{}", std::process::id()));
        let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
        let mut toml = std::fs::read_to_string(&reg).expect("registry");
        std::fs::create_dir_all(ws.join("docs")).expect("docs dir");
        for (file, bytes, pages) in [("Deck.pdf", DECK, 15), ("Mda.pdf", MDA, 80)] {
            std::fs::write(ws.join("docs").join(file), bytes).expect("doc bytes");
            let _ = write!(
                toml,
                "\n[[document]]\nfile = \"{file}\"\ndir = \"docs\"\nkind = \"mda\"\n\
                 url = \"https://example.invalid/{file}\"\nurl_basis = \"W1 table\"\npublished_at = \"2030-02-01\"\n\
                 published_at_basis = \"FACT: synthetic\"\npages = {pages}\nbytes = {}\nsha256 = \"{}\"\n",
                bytes.len(),
                sha256(bytes).to_hex()
            );
        }
        std::fs::write(&reg, toml).expect("registry");
        let path = transcriptions_path(&ws, "synthetic");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(
            &path,
            format!(
                "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n{}",
                rows.concat()
            ),
        )
        .expect("transcriptions");
        ws
    }

    fn entry(reader: &str, doc: &str, page: u64, row: &str, token: &str, quantity: &str) -> String {
        let sha = sha256(if doc == "Deck.pdf" { DECK } else { MDA }).to_hex();
        format!(
            "\n[[entry]]\ndoc = \"{doc}\"\ndoc_sha256 = \"{sha}\"\npage = {page}\ntable = \"Reserves roll-forward\"\n\
             row = \"{row}\"\ncol = \"2030\"\ntoken = \"{token}\"\nquantity = \"{quantity}\"\nunit = \"Moz\"\n\
             basis = \"attributable\"\nperiod = \"2030\"\nreader = \"{reader}\"\n"
        )
    }

    /// E698's shape, signed as a roll-forward prints it: the deck and the MD&A print depletion and net change.
    fn rows() -> Vec<String> {
        let mut rows = Vec::new();
        for reader in ["reader_a", "reader_b"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Depletion",
                "(3.6)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Deck.pdf",
                14,
                "Net change",
                "1.7",
                "net_change",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Depletion",
                "(3.7)",
                "depletion",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                71,
                "Net change",
                "1.8",
                "net_change",
            ));
        }
        rows
    }

    fn run(ws: &Path) -> Result<crate::PublicAssuranceReport, Refusal> {
        run_public_assurance(&load_issuer_profile(ws, "synthetic").expect("registry"), ws)
    }

    #[test]
    fn test_transcribed_tie_discovers_the_deck_convention() {
        let ws = workspace("tie", &rows());
        let r = run(&ws).expect("run");
        let t = r.transcriptions.as_ref().expect("registry present");
        assert_eq!((t.entries, t.agreement.accepted.len()), (8, 4));
        assert_eq!(
            t.documents
                .iter()
                .map(|d| d.file.as_str())
                .collect::<Vec<_>>(),
            ["Deck.pdf", "Mda.pdf"]
        );
        let Some(Ok(outcome)) = &t.tie else {
            panic!("tie: {:?}", t.tie)
        };
        assert!(outcome.conflicts.is_empty());
        let d = outcome.discovery.as_ref().expect("discovery");
        assert_eq!(
            d.findings,
            [
                (
                    "Deck.pdf".to_string(),
                    TieFinding::Identified(Convention::Truncate)
                ),
                (
                    "Mda.pdf".to_string(),
                    TieFinding::Ambiguous(vec![
                        Convention::HalfUp,
                        Convention::HalfDown,
                        Convention::HalfEven
                    ])
                ),
            ]
        );
        // The section names conventions, counts and digests, never a transcribed value.
        let json = transcription_json(Some(t));
        assert!(
            json.contains(
                "\"tie\":{\"conflicts\":[],\"discovery\":{\"documents\":[\"Deck.pdf\",\"Mda.pdf\"],\
                 \"quantities\":[\"depletion\",\"net_change\"],\"assignments\":16,\"fits\":{\"proper\":3,\
                 \"points\":0,\"empty\":13},\"closed_fit\":\"points\",\"findings\":[{\"document\":\"Deck.pdf\",\
                 \"finding\":\"identified\",\"conventions\":[\"truncate\"],\"reason_code\":null},\
                 {\"document\":\"Mda.pdf\",\"finding\":\"ambiguous\",\"conventions\":[\"half_up\",\"half_down\",\
                 \"half_even\"],\"reason_code\":\"FMINE-REF-104\"}]}}"
            ),
            "{json}"
        );
        for value in ["3.6", "3.7", "1.7", "1.8"] {
            assert!(!json.contains(value), "{value} in {json}");
        }
        assert!(
            r.to_json().contains(&json),
            "the payload carries the section"
        );
        // The section is outside the state root: the same run without the registry has the same root.
        std::fs::remove_file(transcriptions_path(&ws, "synthetic")).expect("remove registry");
        let bare = run(&ws).expect("run");
        assert!(bare.transcriptions.is_none());
        assert!(bare.to_json().contains("\"transcriptions\":null,"));
        assert_eq!(bare.ledger_anchor.state_root, r.ledger_anchor.state_root);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_irreconcilable_quantity_is_a_conflict_and_left_out() {
        // E692's shape: the later document prints the prior year's net change as 79, the first as 6.7.
        let mut rows = rows();
        for reader in ["reader_a", "reader_b"] {
            rows.push(entry(
                reader,
                "Deck.pdf",
                13,
                "Prior net change",
                "6.7",
                "net_change_prior",
            ));
            rows.push(entry(
                reader,
                "Mda.pdf",
                70,
                "Prior net change",
                "79",
                "net_change_prior",
            ));
        }
        let ws = workspace("conflict", &rows);
        let r = run(&ws).expect("run");
        let t = r.transcriptions.as_ref().expect("registry present");
        let Some(Ok(outcome)) = &t.tie else {
            panic!("tie: {:?}", t.tie)
        };
        let at: Vec<String> = outcome.conflicts[0]
            .1
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(outcome.conflicts[0].0, "net_change_prior");
        assert_eq!(
            at,
            [
                "Deck.pdf:p13:Reserves roll-forward:Prior net change/2030",
                "Mda.pdf:p70:Reserves roll-forward:Prior net change/2030"
            ]
        );
        let d = outcome
            .discovery
            .as_ref()
            .expect("re-run without the conflict");
        assert_eq!(
            d.findings[0].1,
            TieFinding::Identified(Convention::Truncate)
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_missing_or_altered_document_and_disagreement_refuse() {
        let ws = workspace("refuse", &rows());
        // A pending locator (one reader) is reported, not refused.
        let mut pending = rows();
        pending.push(entry(
            "reader_a",
            "Mda.pdf",
            72,
            "Acquisitions",
            "2.2",
            "acquisitions",
        ));
        let path = transcriptions_path(&ws, "synthetic");
        let header = "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n";
        std::fs::write(&path, format!("{header}{}", pending.concat())).expect("write");
        let t = run(&ws).expect("run").transcriptions.expect("present");
        assert_eq!(t.agreement.single_reader.len(), 1);
        assert!(transcription_json(Some(&t)).contains(
            "\"single_reader\":[\"Mda.pdf:p72:Reserves roll-forward:Acquisitions/2030\"]"
        ));
        // Planted negative: reader_b's depletion in the deck differs. The run refuses, naming the locator and
        // the field, never the values.
        let mut conflict = rows();
        conflict[4] = entry(
            "reader_b",
            "Deck.pdf",
            14,
            "Depletion",
            "(3.5)",
            "depletion",
        );
        std::fs::write(&path, format!("{header}{}", conflict.concat())).expect("write");
        let e = run(&ws).expect_err("disagreement refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-202", 3));
        assert!(
            e.what.contains(
                "at Deck.pdf:p14:Reserves roll-forward:Depletion/2030 (reader_a, reader_b: token)"
            ),
            "{}",
            e.what
        );
        assert!(
            !e.what.contains("3.5") && !e.what.contains("3.6"),
            "{}",
            e.what
        );
        std::fs::write(&path, format!("{header}{}", rows().concat())).expect("write");
        // One altered byte in a cited document.
        let mut altered = DECK.to_vec();
        altered[0] ^= 1;
        std::fs::write(ws.join("docs").join("Deck.pdf"), &altered).expect("alter");
        let e = run(&ws).expect_err("altered refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-505", 8));
        assert!(e.what.contains("digest mismatch: Deck.pdf"), "{}", e.what);
        // A missing cited document.
        std::fs::remove_file(ws.join("docs").join("Deck.pdf")).expect("remove");
        let e = run(&ws).expect_err("missing refuses");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-506", 5));
        assert!(e.what.contains("missing: Deck.pdf"), "{}", e.what);
        // A `--through` run reads no transcriptions, so the missing document does not refuse it.
        let profile = load_issuer_profile(&ws, "synthetic")
            .expect("registry")
            .through("2030Q2")
            .expect("bound");
        assert!(
            run_public_assurance(&profile, &ws)
                .expect("run")
                .transcriptions
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&ws);
    }
}
