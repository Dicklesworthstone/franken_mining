//! DESIGN (BD-347): bind an issued assurance artifact to the typed run that produced it. Registries are
//! read once, then parsed from those same bytes. Workbook and PDF hashes come from the bytes actually
//! checked. The manifest includes optional-file absence and the selected scope; compiled rules and the
//! built-in synthetic profile are separately identified by the issuing binary's digest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fmine_claim::{Claim, ClaimId, Confidentiality};
use fmine_core::anchor::LedgerAnchor;
use fmine_core::{Digest32, Encoder, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_metrics::cutoff;
use fmine_public::sources::{IssuerProfile, is_issuer_name, is_through_bound};
use fmine_public::transcribe::{CheckedDocument, TranscriptionReport, tie_outcome};
use fmine_public::{Lane, PublicAssuranceReport};
use fmine_registry::sources::parse_sources_registry;
use fmine_registry::transcriptions::{agree, parse_transcriptions};

/// A generated report and its issuance bindings. The caller serializes `envelope` once and hashes exactly
/// the bytes it writes; this type does not accept a caller-supplied report or claim inventory.
#[derive(Debug)]
pub(super) struct Prepared {
    pub envelope: crate::Envelope,
    pub claim_ids: Vec<ClaimId>,
    pub input_manifest_root: Digest32,
    pub anchor: LedgerAnchor,
    pub lane: Lane,
    pub taint: Confidentiality,
}

type Inputs = BTreeMap<String, Option<Digest32>>;

fn input(what: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, what)
}

fn text<'a>(bytes: &'a [u8], name: &str) -> Result<&'a str, Refusal> {
    std::str::from_utf8(bytes).map_err(|e| input(format!("{name}: {e}")))
}

fn bind(inputs: &mut Inputs, name: String, digest: Option<Digest32>) -> Result<(), Refusal> {
    if inputs.get(&name).is_some_and(|prior| *prior != digest) {
        return Err(input(format!(
            "{name}: input changed between uses in one issuance"
        )));
    }
    inputs.insert(name, digest);
    Ok(())
}

/// `None` is recorded only for an optional file genuinely absent from the selected run.
fn read(
    root: &Path,
    name: &str,
    optional: bool,
    inputs: &mut Inputs,
) -> Result<Option<Vec<u8>>, Refusal> {
    match std::fs::read(root.join(name)) {
        Ok(bytes) => {
            bind(inputs, name.to_string(), Some(sha256(&bytes)))?;
            Ok(Some(bytes))
        }
        Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => {
            bind(inputs, name.to_string(), None)?;
            Ok(None)
        }
        Err(e) => Err(input(format!("{name}: {e}"))),
    }
}

fn transcription(
    root: &Path,
    profile: &IssuerProfile,
    inputs: &mut Inputs,
) -> Result<Option<TranscriptionReport>, Refusal> {
    if profile.through.is_some() {
        return Ok(None);
    }
    let name = format!("registries/transcriptions/{}.toml", profile.issuer);
    let Some(bytes) = read(root, &name, true, inputs)? else {
        return Ok(None);
    };
    let registry = parse_transcriptions(text(&bytes, &name)?, &profile.issuer, &profile.documents)
        .map_err(|e| input(format!("{name}: {e}")))?;
    let mut documents = Vec::new();
    for d in &profile.documents {
        if !registry.entries.iter().any(|e| e.locator.doc == d.file) {
            continue;
        }
        let path = format!("{}/{}", d.dir, d.file);
        let bytes = read(root, &path, false, inputs)?
            .ok_or_else(|| input(format!("{path}: missing document")))?;
        let actual = sha256(&bytes);
        if bytes.len() as u64 != d.bytes || actual.to_hex() != d.sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("{path}: registered document bytes or SHA-256 differ"),
            ));
        }
        documents.push(CheckedDocument {
            file: d.file.clone(),
            kind: d.kind.clone(),
            published_at: d.published_at.clone(),
            published_at_basis: d.published_at_basis.clone(),
            sha256: actual.to_hex(),
        });
    }
    let agreement = agree(&registry);
    if !agreement.is_consistent() {
        let locators = agreement
            .disagreements
            .iter()
            .map(|d| d.locator.to_string())
            .chain(
                agreement
                    .quantity_clashes
                    .iter()
                    .flat_map(|(_, _, at)| at.iter().map(ToString::to_string)),
            )
            .collect::<Vec<_>>()
            .join(", ");
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            format!("{name}: readers disagree at {locators}"),
        ));
    }
    Ok(Some(TranscriptionReport {
        registry: name,
        documents,
        entries: registry.entries.len(),
        tie: tie_outcome(&agreement.accepted),
        agreement,
    }))
}

fn add<T>(
    claim: &Claim<T>,
    ids: &mut BTreeSet<ClaimId>,
    taint: &mut Confidentiality,
) -> Result<(), Refusal> {
    claim.verify_export_clearance(&Confidentiality::public())?;
    if claim.stale {
        return Err(Refusal::new(
            RefusalCode::NotObservable202,
            format!(
                "issued assurance contains explicitly stale Claim {}",
                claim.claim_id
            ),
        ));
    }
    claim.check_id()?;
    *taint = taint.join(&claim.confidentiality);
    ids.insert(claim.claim_id);
    ids.extend(claim.parents.iter().copied());
    Ok(())
}

/// All emitted Claim populations, including source references and optional model outputs. No epistemic
/// upgrade is performed: uncalibrated recovery models remain descriptive in the unchanged report.
fn claims(report: &PublicAssuranceReport) -> Result<(Vec<ClaimId>, Confidentiality), Refusal> {
    let (mut ids, mut taint) = (BTreeSet::new(), Confidentiality::public());
    let cells = report
        .public_claims
        .cells
        .values()
        .chain(report.mine_stats_claims.claims.iter())
        .chain(report.waterfall.pairs.iter().flat_map(|p| &p.inputs))
        .chain(
            [&report.aisc_vintages, &report.mine_stats_vintages]
                .into_iter()
                .flat_map(|v| &v.items)
                .flat_map(|i| i.first.iter().chain(&i.previous).chain(&i.current)),
        );
    for cell in cells {
        add(&cell.claim, &mut ids, &mut taint)?;
    }
    if let Some(r) = &report.recovery {
        add(&r.ownership, &mut ids, &mut taint)?;
        add(&r.fit, &mut ids, &mut taint)?;
        for row in &r.training {
            add(&row.head, &mut ids, &mut taint)?;
            add(&row.recovery, &mut ids, &mut taint)?;
        }
        for q in &r.quarters {
            for cell in &q.inputs {
                add(&cell.claim, &mut ids, &mut taint)?;
            }
            if let Some(row) = &q.operating {
                for claim in [&row.head, &row.recovery, &row.processed, &row.produced] {
                    add(claim, &mut ids, &mut taint)?;
                }
            }
            if let Ok(p) = &q.outcome {
                add(&p.constant_tails, &mut ids, &mut taint)?;
                add(&p.grade_dependent_tails, &mut ids, &mut taint)?;
            }
        }
    }
    Ok((ids.into_iter().collect(), taint))
}

fn finish(
    report: PublicAssuranceReport,
    inputs: Inputs,
    cutoffs: Option<&str>,
) -> Result<Prepared, Refusal> {
    let (claim_ids, taint) = claims(&report)?;
    let envelope = crate::report_envelope(&report, cutoffs);
    if let Some(refusal) = &envelope.refusal {
        return Err(refusal.clone());
    }
    let mut manifest = Encoder::new_frame(*b"RINP", 1);
    manifest.put_str(report.lane.as_str());
    manifest.put_str(&report.issuer);
    manifest.put_str(report.through.as_deref().unwrap_or(""));
    manifest.put_u64(inputs.len() as u64);
    for (path, digest) in inputs {
        manifest.put_str(&path);
        manifest.put_u8(u8::from(digest.is_some()));
        if let Some(digest) = digest {
            manifest.put_fixed(&digest.0);
        }
    }
    Ok(Prepared {
        envelope,
        claim_ids,
        input_manifest_root: blake3_hash(&manifest.finish()),
        anchor: report.ledger_anchor,
        lane: report.lane,
        taint,
    })
}

/// Generates the same assurance envelope from one captured set of registry bytes. `None` selects only the
/// built-in synthetic lane; a registered issuer named `synthetic` retains that lane too.
pub(super) fn prepare(
    root: &Path,
    issuer: Option<&str>,
    through: Option<&str>,
) -> Result<Prepared, Refusal> {
    if through.is_some_and(|q| !is_through_bound(q)) || issuer.is_none() && through.is_some() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "issuance requires a valid issuer-scoped through quarter",
        ));
    }
    let mut inputs = Inputs::new();
    let Some(issuer) = issuer else {
        let report = fmine_public::run_synthetic_assurance(&[])?;
        for s in &report.sources {
            bind(
                &mut inputs,
                format!("synthetic/{}", s.file_name),
                Some(Digest32(s.workbook.sha256)),
            )?;
        }
        return finish(report, inputs, None);
    };
    if !is_issuer_name(issuer) {
        return Err(input("invalid issuer registry name"));
    }
    let name = format!("registries/sources/{issuer}.toml");
    let bytes = read(root, &name, false, &mut inputs)?
        .ok_or_else(|| input(format!("{name}: missing registry")))?;
    let registry =
        parse_sources_registry(text(&bytes, &name)?).map_err(|e| input(format!("{name}: {e}")))?;
    if registry.issuer != issuer {
        return Err(input(format!(
            "{name}: issuer does not match registry name"
        )));
    }
    let mut profile = IssuerProfile::from_registry(registry).map_err(input)?;
    if let Some(bound) = through {
        profile = profile.through(bound)?;
    }
    let cutoffs = read(root, cutoff::CUTOFFS_REGISTRY, true, &mut inputs)?
        .map(|bytes| cutoff::section(&bytes).map_err(|e| input(e.to_string())))
        .transpose()?
        .filter(|s| s.issuer == issuer);
    let sources = fmine_public::load_registered_sources(&profile, root)?;
    for s in &sources {
        bind(
            &mut inputs,
            format!("{}/{}", profile.corpus_dir, s.file_name),
            Some(Digest32(s.workbook.sha256)),
        )?;
    }
    let transcriptions = transcription(root, &profile, &mut inputs)?;
    let lane = if issuer == fmine_public::synthetic::SYNTHETIC_ISSUER {
        Lane::Synthetic
    } else {
        Lane::Public
    };
    let mut report = fmine_public::assess(&profile, lane, sources);
    report.recovery = fmine_public::recovery::assess(
        &profile,
        transcriptions.as_ref(),
        &report.mine_stats_claims.claims,
    )?;
    report.transcriptions = transcriptions;
    finish(report, inputs, cutoffs.as_ref().map(|s| s.json.as_str()))
}

#[cfg(test)]
#[path = "../../xtask/testdata/reliance_binding_controls.rs"]
mod tests;
