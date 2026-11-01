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
        envelope: crate::report_envelope(&report, cutoffs),
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
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn workspace() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fmine-issuance-binding-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fmine_public::synthetic::write_issuer_workspace(&root).expect("synthetic workspace");
        root
    }

    #[test]
    fn synthetic_and_registered_bindings_keep_the_existing_envelope_and_sorted_claims() {
        let root = workspace();
        for issuer in [None, Some("synthetic")] {
            let p = prepare(&root, issuer, None).expect("prepared");
            let report = match issuer {
                None => fmine_public::run_synthetic_assurance(&[]).expect("synthetic"),
                Some(i) => {
                    let profile = fmine_public::load_issuer_profile(&root, i).expect("profile");
                    fmine_public::run_public_assurance(&profile, &root).expect("registered")
                }
            };
            assert_eq!(p.envelope, crate::report_envelope(&report, None));
            assert_eq!(p.anchor, report.ledger_anchor);
            assert_eq!(p.lane, Lane::Synthetic);
            assert!(p.taint.is_public());
            assert!(!p.claim_ids.is_empty());
            assert!(p.claim_ids.windows(2).all(|w| w[0] < w[1]));
            assert_eq!(
                p.input_manifest_root,
                prepare(&root, issuer, None)
                    .expect("rerun")
                    .input_manifest_root
            );
        }
    }

    #[test]
    fn manifest_binds_registry_bytes_cutoff_presence_and_actual_source_content() {
        let root = workspace();
        let first = prepare(&root, Some("synthetic"), None).expect("first");
        let registry = root.join("registries/sources/synthetic.toml");
        let mut bytes = std::fs::read(&registry).expect("registry");
        bytes.extend_from_slice(b"\n# A different byte snapshot with the same interpretation.\n");
        std::fs::write(&registry, &bytes).expect("comment");
        let second = prepare(&root, Some("synthetic"), None).expect("second");
        assert_eq!(first.envelope, second.envelope);
        assert_ne!(first.input_manifest_root, second.input_manifest_root);
        std::fs::write(
            root.join(cutoff::CUTOFFS_REGISTRY),
            "schema = \"fmine.published_cutoffs.v1\"\nissuer = \"synthetic\"\n",
        )
        .expect("cutoffs");
        let third = prepare(&root, Some("synthetic"), None).expect("cutoff section");
        assert_ne!(second.input_manifest_root, third.input_manifest_root);
        assert_eq!(second.anchor, third.anchor);
        assert!(third.envelope.payload_json.contains("published_cutoffs"));
        std::fs::write(root.join("corpus/Synthetic_AISC.xlsx"), b"altered")
            .expect("source mutation");
        assert_eq!(
            prepare(&root, Some("synthetic"), None)
                .expect_err("altered source")
                .code,
            "FMINE-REF-505"
        );
        let mut inputs = Inputs::new();
        bind(&mut inputs, "same-file".into(), None).expect("absent input");
        assert!(bind(&mut inputs, "same-file".into(), Some(sha256(b"later"))).is_err());
    }

    #[test]
    fn optional_recovery_claims_and_references_are_complete_without_label_downgrades() {
        let mut report = fmine_public::run_synthetic_assurance(&[]).expect("base");
        let base = claims(&report).expect("base claims").0;
        report.recovery = Some(fmine_public::recovery::run_synthetic().expect("recovery"));
        let bound = claims(&report).expect("all claims").0;
        let json = report.to_json();
        let printed: BTreeSet<ClaimId> = json
            .match_indices("clm_")
            .map(|(at, _)| json[at..at + 36].parse().expect("printed Claim id"))
            .collect();
        assert_eq!(bound.iter().copied().collect::<BTreeSet<_>>(), printed);
        assert!(bound.len() > base.len());
        let r = report.recovery.as_mut().expect("recovery");
        r.fit.confidentiality = Confidentiality::internal();
        assert_eq!(
            claims(&report).expect_err("private output").code,
            "FMINE-REF-301"
        );
        let r = report.recovery.as_mut().expect("recovery");
        r.fit.confidentiality = Confidentiality::public();
        r.fit.stale = true;
        assert_eq!(
            claims(&report).expect_err("stale output").code,
            "FMINE-REF-202"
        );
        let r = report.recovery.as_mut().expect("recovery");
        r.fit.stale = false;
        r.fit.metric_key.push_str(".mutated");
        assert_eq!(
            claims(&report).expect_err("mutated output").code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn pdf_transcriptions_use_captured_registry_bytes_and_the_same_tie_engine() {
        let root = workspace();
        let path = root.join("registries/sources/synthetic.toml");
        let mut sources = std::fs::read_to_string(&path).expect("registry");
        let mut entries =
            "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
        for name in ["one.pdf", "two.pdf"] {
            let bytes = format!("Invented document {name}");
            let digest = sha256(bytes.as_bytes()).to_hex();
            std::fs::write(root.join("corpus").join(name), &bytes).expect("document");
            sources.push_str(&format!("\n[[document]]\nfile = \"{name}\"\nkind = \"mda\"\nurl = \"https://example.invalid/{name}\"\nurl_basis = \"W1 table\"\npublished_at = \"2030-01-01\"\npublished_at_basis = \"FACT: invented fixture date\"\npages = 1\nbytes = {}\nsha256 = \"{digest}\"\n", bytes.len()));
            for reader in ["reader-a", "reader-b"] {
                entries.push_str(&format!("\n[[entry]]\ndoc = \"{name}\"\ndoc_sha256 = \"{digest}\"\npage = 1\ntable = \"Invented\"\nrow = \"total\"\ncol = \"amount\"\ntoken = \"10\"\nquantity = \"synthetic_total\"\nunit = \"usd_m\"\nbasis = \"100%\"\nperiod = \"2030FY\"\nreader = \"{reader}\"\n"));
            }
        }
        std::fs::write(&path, sources).expect("document registry");
        std::fs::create_dir_all(root.join("registries/transcriptions")).expect("transcription dir");
        let path = root.join("registries/transcriptions/synthetic.toml");
        std::fs::write(&path, &entries).expect("transcriptions");
        let first = prepare(&root, Some("synthetic"), None).expect("transcribed");
        let profile = fmine_public::load_issuer_profile(&root, "synthetic").expect("profile");
        let report = fmine_public::run_public_assurance(&profile, &root).expect("same pipeline");
        assert!(
            report
                .transcriptions
                .as_ref()
                .expect("transcription report")
                .tie
                .is_some()
        );
        assert_eq!(first.envelope, crate::report_envelope(&report, None));
        let bounded = prepare(&root, Some("synthetic"), Some("2030Q1")).expect("bounded");
        std::fs::write(&path, entries.replace("token = \"10\"", "token = \"11\""))
            .expect("changed transcriptions");
        let second = prepare(&root, Some("synthetic"), None).expect("changed");
        assert_eq!(first.anchor, second.anchor);
        assert_ne!(first.input_manifest_root, second.input_manifest_root);
        assert_ne!(first.envelope, second.envelope);
        assert_eq!(
            bounded.input_manifest_root,
            prepare(&root, Some("synthetic"), Some("2030Q1"))
                .expect("still bounded")
                .input_manifest_root
        );
        std::fs::write(root.join("corpus/one.pdf"), b"altered").expect("altered PDF");
        assert_eq!(
            prepare(&root, Some("synthetic"), None)
                .expect_err("altered PDF")
                .code,
            "FMINE-REF-505"
        );
    }
}
