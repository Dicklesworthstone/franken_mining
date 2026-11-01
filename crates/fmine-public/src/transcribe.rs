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
use fmine_registry::sources::is_date;
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
pub fn tie_outcome(accepted: &[Accepted]) -> Option<Result<TieOutcome, String>> {
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
    load_report(profile, root, None, false)
}

/// Loads only rows whose registered document was public by `as_of`. Future documents' bytes and
/// reader agreements cannot block an earlier evidence cut. Registry syntax and document identities
/// remain checked; the general assurance loader above still checks every cited document.
pub fn load_transcription_report_as_of(
    profile: &IssuerProfile,
    root: &Path,
    as_of: &str,
) -> Result<Option<TranscriptionReport>, Refusal> {
    if !fmine_registry::sources::is_date(as_of) {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "transcription as-of needs a valid YYYY-MM-DD date",
        ));
    }
    load_report(profile, root, Some(as_of), false)
}

/// Loads a guidance report while retaining incomplete optional transcriptions as unaccepted evidence.
/// Guidance disagreements and unknown pending quantity identities still refuse. Every cited document's
/// bytes remain checked; the general assurance loaders retain their all-quantity agreement gate.
pub fn load_guidance_transcription_report(
    profile: &IssuerProfile,
    root: &Path,
    as_of: Option<&str>,
) -> Result<Option<TranscriptionReport>, Refusal> {
    if as_of.is_some_and(|cut| !is_date(cut)) {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "as-of must be YYYY-MM-DD",
        ));
    }
    load_report(profile, root, as_of, true)
}

fn load_report(
    profile: &IssuerProfile,
    root: &Path,
    as_of: Option<&str>,
    guidance_only: bool,
) -> Result<Option<TranscriptionReport>, Refusal> {
    let path = transcriptions_path(root, &profile.issuer);
    if !path.is_file() {
        return Ok(None);
    }
    let registry = format!("registries/transcriptions/{}.toml", profile.issuer);
    let mut reg = load_transcriptions(&path, &profile.issuer, &profile.documents)
        .map_err(|e| crate::input_refusal(format!("transcription registry {registry}: {e}")))?;
    if let Some(cut) = as_of {
        reg.entries.retain(|e| {
            profile
                .documents
                .iter()
                .any(|d| d.file == e.locator.doc && d.published_at.as_str() <= cut)
        });
    }
    let cited: Vec<&DocumentRow> = profile
        .documents
        .iter()
        .filter(|d| reg.entries.iter().any(|e| e.locator.doc == d.file))
        .collect();
    let documents = check_documents(&cited, root)?;
    let agreement = agree(&reg);
    if !agreement.is_consistent()
        && !(guidance_only && agreement.is_complete_for_prefix("guidance_"))
    {
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

// Cross-crate transcription checks live in xtask/tests/public_transcriptions.rs.
