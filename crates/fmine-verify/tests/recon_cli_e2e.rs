//! End-to-end tests for `BD-162` (`fmine recon run` matched-cohort + posterior, `fmine recon close`
//! attested period close verified offline by `fmine-verify cert --inputs-dir`) and `BD-174`
//! (`fmine drill verify-index` technical-report data-verification index skeleton).

use std::fs;
use std::path::{Path, PathBuf};

use fmine_recon::certificate::{AttestationStatus, ReconciliationCertificate};
use fmine_verify::run_cli as run_verify_cli;

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[test]
fn test_bd162_recon_run_and_attested_close_verified_by_fmine_verify() {
    let root = workspace_root();
    let work = std::env::temp_dir().join(format!("fmine_bd162_e2e_{}", std::process::id()));
    let _guard = TempDirGuard(work.clone());
    fs::create_dir_all(&work).expect("create temp dir");

    // 1. Existing matched-cohort `fmine recon run` remains green and backwards-compatible.
    let (mc_out, mc_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            "tests/recon/matched_cohorts.toml".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(mc_code, 0, "matched-cohort recon run failed: {mc_out}");
    assert!(mc_out.contains("fmine.matched-cohort-result.v1"));

    // 2. Posterior `fmine recon run` on GB-24 + GB-25 fixture computes exact WLS + F2 + VOI and
    //    emits a replay-stable `review_target_digest` without writing files.
    let (run_a, code_a) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            "tests/recon/posterior_gb24.toml".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(code_a, 0, "posterior recon run failed: {run_a}");
    assert!(run_a.contains("fmine.recon-posterior-result.v1"));
    assert!(run_a.contains("\"disposition\":\"admit\""));
    assert!(run_a.contains("\"refusal_code\":\"FMINE-REF-104\""));
    assert!(run_a.contains("mill_stopped_belt_audit"));
    assert!(run_a.contains("\"reserve_disclosure_claim\":false"));

    let (run_b, code_b) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            "tests/recon/posterior_gb24.toml".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(code_b, 0);
    assert_eq!(run_a, run_b, "posterior recon run must be byte-identical");

    let review_digest =
        extract_json_str(&run_a, "review_target_digest").expect("review_target_digest in JSON");
    assert_eq!(review_digest.len(), 64);

    // 3. `fmine recon close` with stale/wrong `--against-digest` refuses with FMINE-REF-505 (exit 8)
    //    and writes ZERO files.
    let bad_digest_dir = work.join("close_bad_digest");
    let (bad_out, bad_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "close".to_string(),
            "--input".to_string(),
            "tests/recon/posterior_gb24.toml".to_string(),
            "--out-dir".to_string(),
            bad_digest_dir.to_string_lossy().to_string(),
            "--reviewer-role".to_string(),
            "Site Metallurgical Accountant".to_string(),
            "--against-digest".to_string(),
            "0".repeat(64),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(bad_code, 8, "stale digest must exit 8: {bad_out}");
    assert!(bad_out.contains("FMINE-REF-505"));
    assert!(
        !bad_digest_dir.exists(),
        "refused close must write no files"
    );

    // 4. `fmine recon close` with exact reviewed digest seals RCRT + FMT-CERT-1 + input closure.
    let close_dir = work.join("close_ok");
    let (close_out, close_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "close".to_string(),
            "--input".to_string(),
            "tests/recon/posterior_gb24.toml".to_string(),
            "--out-dir".to_string(),
            close_dir.to_string_lossy().to_string(),
            "--reviewer-role".to_string(),
            "Site Metallurgical Accountant".to_string(),
            "--against-digest".to_string(),
            review_digest.clone(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(close_code, 0, "attested recon close failed: {close_out}");
    assert!(close_out.contains("fmine.recon-close-result.v1"));
    assert!(close_out.contains("\"status\":\"reviewed_against\""));

    let rcrt_path = close_dir.join("recon_certificate.rcrt");
    let cert_path = close_dir.join("recon_certificate.cert");
    let inputs_dir = close_dir.join("inputs");
    assert!(rcrt_path.exists());
    assert!(cert_path.exists());
    assert!(close_dir.join("recon_certificate.md").exists());

    // Decode RCRT directly and confirm attestation & INV-002 flags.
    let decoded = ReconciliationCertificate::from_bytes(&fs::read(&rcrt_path).expect("read rcrt"))
        .expect("decode RCRT");
    assert!(!decoded.reserve_disclosure_claim());
    assert!(!decoded.execution_authorized());
    assert!(!decoded.geotechnical_signoff());
    assert!(!decoded.regulatory_compliance_claim());
    assert_eq!(
        decoded.attestation,
        AttestationStatus::ReviewedAgainst {
            reviewer_role: "Site Metallurgical Accountant".to_string(),
            against_digest_hex: review_digest.clone(),
        }
    );

    // 5. Offline `fmine-verify cert <cert> --inputs-dir <inputs>` exits 0!
    let (ver_ok, ver_code) = run_verify_cli(&[
        "cert".to_string(),
        cert_path.to_string_lossy().to_string(),
        "--inputs-dir".to_string(),
        inputs_dir.to_string_lossy().to_string(),
    ]);
    assert_eq!(
        ver_code, 0,
        "fmine-verify must accept closed cert: {ver_ok}"
    );
    assert!(ver_ok.contains("\"status\":\"verified\""));
    assert!(ver_ok.contains("\"subject_kind\":\"touchstone_reconciliation\""));

    // 6. Append-only protection: re-closing into the same --out-dir refuses.
    let (dup_out, dup_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "close".to_string(),
            "--input".to_string(),
            "tests/recon/posterior_gb24.toml".to_string(),
            "--out-dir".to_string(),
            close_dir.to_string_lossy().to_string(),
            "--reviewer-role".to_string(),
            "Site Metallurgical Accountant".to_string(),
            "--against-digest".to_string(),
            review_digest,
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(dup_code, 0, "overwrite must refuse: {dup_out}");
    assert!(dup_out.contains("FMINE-REF-506"));

    // 7. Single-bit tamper of the sealed FMT-CERT-1 exits 8 (FMINE-REF-505).
    let tampered_path = work.join("tampered.cert");
    let mut bytes = fs::read(&cert_path).expect("read cert");
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x01;
    fs::write(&tampered_path, &bytes).expect("write tampered");
    let (ver_bad, ver_bad_code) = run_verify_cli(&[
        "cert".to_string(),
        tampered_path.to_string_lossy().to_string(),
    ]);
    assert_eq!(ver_bad_code, 8, "tampered cert must exit 8: {ver_bad}");
    assert!(ver_bad.contains("FMINE-REF-505"));

    // 8. INV-005 gross-error period: `recon close` refuses with FMINE-REF-101 (exit 3) and writes ZERO files.
    let bad_meas_toml = fs::read_to_string(root.join("tests/recon/posterior_gb24.toml"))
        .expect("read fixture")
        .replace("observed = \"1000000\"", "observed = \"1250000\""); // +250kt gross error (> 7 sigma)!
    let ge_input = work.join("gross_error.toml");
    fs::write(&ge_input, &bad_meas_toml).expect("write gross error input");
    let ge_out_dir = work.join("close_gross_error");
    let (ge_out, ge_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "close".to_string(),
            "--input".to_string(),
            ge_input.to_string_lossy().to_string(),
            "--out-dir".to_string(),
            ge_out_dir.to_string_lossy().to_string(),
            "--allow-draft".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(
        ge_code, 3,
        "gross-error close must exit 3 (FMINE-REF-101): {ge_out}"
    );
    assert!(ge_out.contains("FMINE-REF-101"));
    assert!(
        !ge_out_dir.exists(),
        "gross-error refusal must write zero files"
    );

    // 9. Guard G-TOUCH-1: direct grade channel refuses with FMINE-REF-103.
    let grade_toml = fs::read_to_string(root.join("tests/recon/posterior_gb24.toml"))
        .expect("read fixture")
        .replacen("channel = \"dry_mass\"", "channel = \"grade\"", 1);
    let grade_input = work.join("grade_refuse.toml");
    fs::write(&grade_input, &grade_toml).expect("write grade input");
    let (gr_out, gr_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            grade_input.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(gr_code, 0);
    assert!(
        gr_out.contains("FMINE-REF-103"),
        "G-TOUCH-1 must return FMINE-REF-103: {gr_out}"
    );

    // 10. Guard G-TOUCH-4: empty F-factor definition_version refuses with FMINE-REF-107.
    let no_ver_toml = fs::read_to_string(root.join("tests/recon/posterior_gb24.toml"))
        .expect("read fixture")
        .replace("definition_version = \"v1.0\"", "definition_version = \"\"");
    let no_ver_input = work.join("no_ff_ver.toml");
    fs::write(&no_ver_input, &no_ver_toml).expect("write no_ver input");
    let (nv_out, nv_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            no_ver_input.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(nv_code, 0);
    assert!(
        nv_out.contains("FMINE-REF-107"),
        "G-TOUCH-4 must return FMINE-REF-107: {nv_out}"
    );

    // 11. Duplicate terms in a constraint, period_index overflow, and long site/period filenames.
    let dup_terms_toml = fs::read_to_string(root.join("tests/recon/posterior_gb24.toml"))
        .expect("read fixture")
        .replace(
            "terms = \"truck_t:1, mill_t:-1, sp_delta_t:-1\"",
            "terms = \"truck_t:1, truck_t:-1, mill_t:-1\"",
        );
    let dup_terms_path = work.join("dup_terms.toml");
    fs::write(&dup_terms_path, &dup_terms_toml).expect("write dup_terms");
    let (dt_out, dt_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "run".to_string(),
            "--input".to_string(),
            dup_terms_path.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(dt_code, 0, "duplicate terms must refuse: {dt_out}");
    assert!(dt_out.contains("FMINE-REF-506"));

    // Long site and period names produce a bounded input_locator that closes & verifies cleanly.
    let long_site = "S".repeat(200);
    let long_period = "P".repeat(200);
    let long_toml = fs::read_to_string(root.join("tests/recon/posterior_gb24.toml"))
        .expect("read fixture")
        .replace(
            "site = \"synthetic_gb24_open_pit\"",
            &format!("site = \"{long_site}\""),
        )
        .replace(
            "period = \"2026-06\"",
            &format!("period = \"{long_period}\""),
        );
    let long_path = work.join("long_names.toml");
    fs::write(&long_path, &long_toml).expect("write long_names");
    let long_close_dir = work.join("close_long_names");
    let (lc_out, lc_code) = fmine_cli::run(
        &[
            "recon".to_string(),
            "close".to_string(),
            "--input".to_string(),
            long_path.to_string_lossy().to_string(),
            "--out-dir".to_string(),
            long_close_dir.to_string_lossy().to_string(),
            "--allow-draft".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(lc_code, 0, "long site/period close must succeed: {lc_out}");
    let (lv_out, lv_code) = run_verify_cli(&[
        "cert".to_string(),
        long_close_dir
            .join("recon_certificate.cert")
            .to_string_lossy()
            .to_string(),
        "--inputs-dir".to_string(),
        long_close_dir.join("inputs").to_string_lossy().to_string(),
    ]);
    assert_eq!(
        lv_code, 0,
        "fmine-verify must verify bounded long-name cert: {lv_out}"
    );
}

#[test]
fn test_bd174_cli_drill_verify_index_renders_deterministically() {
    let root = workspace_root();
    let (json_a, code_a) = fmine_cli::run(
        &[
            "drill".to_string(),
            "verify-index".to_string(),
            "--input".to_string(),
            "tests/drill/data_verification_index.toml".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(code_a, 0, "drill verify-index --json failed: {json_a}");
    assert!(json_a.contains("fmine.data-verification-index-result.v1"));
    assert!(json_a.contains("DRAFT STANDARD"));
    assert!(json_a.contains("GAP-RULES-ONLY-2010-2015-LEGACY"));
    assert!(json_a.contains("GAP-DENSITY-LAB-CHECK-E382"));
    assert!(json_a.contains("GAP-DENSITY-GRID-E383"));
    assert!(json_a.contains("FIND-E164-DQO"));
    assert!(json_a.contains("\"reserve_disclosure_claim\":false"));

    let (json_b, code_b) = fmine_cli::run(
        &[
            "drill".to_string(),
            "verify-index".to_string(),
            "--input".to_string(),
            "tests/drill/data_verification_index.toml".to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_eq!(code_b, 0);
    assert_eq!(json_a, json_b, "drill verify-index must be byte-identical");

    let (human, code_h) = fmine_cli::run(
        &[
            "drill".to_string(),
            "verify-index".to_string(),
            "--input".to_string(),
            "tests/drill/data_verification_index.toml".to_string(),
            "--format".to_string(),
            "markdown".to_string(),
        ],
        &root,
    );
    assert_eq!(code_h, 0, "drill verify-index human render failed: {human}");
    assert!(human.contains("## Item 10"));
    assert!(human.contains("## Item 11"));
    assert!(human.contains("## Item 12"));
    assert!(human.contains("## Item 14"));
    assert!(human.contains("DRAFT STANDARD — NOT IN FORCE"));
    assert!(human.contains("FIND-E164-DQO"));
    assert!(human.contains("ALS-Primary"));

    // Underflowing float token ("1e-400") and duplicate original_refs must refuse cleanly.
    let work = std::env::temp_dir().join(format!("fmine_bd174_e2e_{}", std::process::id()));
    let _guard = TempDirGuard(work.clone());
    fs::create_dir_all(&work).expect("create temp dir");

    let base = fs::read_to_string(root.join("tests/drill/data_verification_index.toml"))
        .expect("read verify-index fixture");
    let underflow_toml = base.replace("cv_avr_pct = \"6.1\"", "cv_avr_pct = \"1e-400\"");
    let uf_path = work.join("underflow.toml");
    fs::write(&uf_path, &underflow_toml).expect("write underflow");
    let (uf_out, uf_code) = fmine_cli::run(
        &[
            "drill".to_string(),
            "verify-index".to_string(),
            "--input".to_string(),
            uf_path.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(uf_code, 0, "underflowing float must refuse: {uf_out}");
    assert!(uf_out.contains("FMINE-REF-506"));

    let dup_ref_toml = base.replace(
        "original_refs = \"CERT-2025-01, SURV-2025-01\"",
        "original_refs = \"CERT-2025-01, CERT-2025-01\"",
    );
    let dr_path = work.join("dup_ref.toml");
    fs::write(&dr_path, &dup_ref_toml).expect("write dup_ref");
    let (dr_out, dr_code) = fmine_cli::run(
        &[
            "drill".to_string(),
            "verify-index".to_string(),
            "--input".to_string(),
            dr_path.to_string_lossy().to_string(),
            "--json".to_string(),
        ],
        &root,
    );
    assert_ne!(dr_code, 0, "duplicate original_refs must refuse: {dr_out}");
    assert!(dr_out.contains("FMINE-REF-506"));
}
