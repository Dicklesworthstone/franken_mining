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
    for (format, name) in [
        (crate::OutputFormat::Json, "json"),
        (crate::OutputFormat::Markdown, "markdown"),
    ] {
        let args = [
            "assure",
            "public",
            "--issuer",
            "synthetic",
            "--format",
            name,
        ]
        .map(str::to_string);
        let (preview, exit) = crate::run(&args, &root);
        assert_eq!(exit, 0);
        assert_eq!(third.envelope.render(format), preview);
    }
    assert!(
        third
            .envelope
            .render(crate::OutputFormat::Markdown)
            .contains("## Published cut-offs: reference-input arithmetic")
    );
    std::fs::write(root.join("corpus/Synthetic_AISC.xlsx"), b"altered").expect("source mutation");
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
fn refused_report_rendering_cannot_be_prepared_for_issuance() {
    let mut report = fmine_public::run_synthetic_assurance(&[]).expect("synthetic report");
    let instance = report
        .instances
        .iter_mut()
        .find(|i| {
            i.subject == "Site B" && i.family == fmine_public::aisc::IdentityFamily::TccFooting
        })
        .expect("discrepant source identity");
    instance.operands[0].value = fmine_public::aisc::OperandValue::Int(999);
    assert_eq!(
        finish(report, Inputs::new(), None)
            .expect_err("source packet integrity refusal must block issuance")
            .code,
        "FMINE-REF-208"
    );
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
    let mut entries = "schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n".to_string();
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
