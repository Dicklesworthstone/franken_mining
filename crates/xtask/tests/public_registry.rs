//! Live dependency on the source registry: an altered registered file is refused with both digests, never read.
//! The registry itself is read from `registries/sources/barrick.toml`, copied into a scratch workspace.

use std::fs;
use std::path::PathBuf;

use fmine_public::run_barrick_m0_assurance;
use fmine_public::sources::{load_issuer_profile, registry_path};
use fmine_public::{Lane, run_public_assurance, sources, synthetic};

/// A complete run through a TOML registry in default CI (the real-corpus runs are ignored tests): the two
/// synthetic workbooks plus a third, later Mine Stats file are written to a scratch corpus, registered by
/// digest in a generated TOML, and loaded with no code-side profile. The ledger equals the in-code synthetic
/// profile's, and `--through 2030Q1` drops the third file and leaves the ledger unchanged.
#[test]
fn test_synthetic_three_file_toml_profile_runs_end_to_end() {
    let state_codes = ["i", "ii", "iii", "iv", "v", "np"];
    let ws = std::env::temp_dir().join(format!("fmine-public-toml-e2e-{}", std::process::id()));
    let reg = synthetic::write_issuer_workspace(&ws).expect("write workspace");
    assert_eq!(reg, sources::registry_path(&ws, "synthetic"));
    let code_profile = synthetic::synthetic_profile();
    let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
    assert_eq!(profile.sources.len(), 3);
    assert_eq!(profile.reconstructions, code_profile.reconstructions);
    assert_eq!(profile.missing_facts, code_profile.missing_facts);
    let r = run_public_assurance(&profile, &ws).expect("all three files verify by digest");
    let totals: Vec<usize> = state_codes.iter().map(|c| r.count_state(c)).collect();
    assert_eq!(
        totals,
        vec![40, 2, 1, 2, 5, 5],
        "same ledger as the in-code synthetic profile"
    );
    assert_eq!(r.hidden_scope.instances.len(), 5);
    assert_eq!(
        (&profile.regions, &profile.shares),
        (&code_profile.regions, &code_profile.shares)
    );
    assert_eq!(r.sources.len(), 3);
    assert_eq!(r.ledger_anchor.commit_sequence, 3);
    // The reserved issuer name keeps invented numbers out of the public lane; the same registry under
    // another name would be gate-eligible, so the name is what decides.
    assert_eq!((r.lane, r.gate_eligible()), (Lane::Synthetic, false));
    let mut renamed = profile.clone();
    renamed.issuer = "renamed".to_string();
    let public = run_public_assurance(&renamed, &ws).expect("same files");
    assert_eq!((public.lane, public.gate_eligible()), (Lane::Public, true));
    let bounded =
        run_public_assurance(&profile.through("2030Q1").expect("bound"), &ws).expect("run");
    assert_eq!(bounded.sources.len(), 2);
    assert_eq!(bounded.through.as_deref(), Some("2030Q1"));
    let totals: Vec<usize> = state_codes.iter().map(|c| bounded.count_state(c)).collect();
    assert_eq!(totals, vec![40, 2, 1, 2, 5, 5]);
    assert_ne!(
        bounded.ledger_anchor.state_root, r.ledger_anchor.state_root,
        "the source set is in the root"
    );
    let _ = std::fs::remove_dir_all(&ws);
}

#[test]
fn altered_registered_file_is_refused_with_both_digests() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let profile = load_issuer_profile(&repo, "barrick").expect("registry");
    let n = profile.sources.len();

    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("altered-workspace");
    let reg = registry_path(&root, "barrick");
    fs::create_dir_all(reg.parent().expect("registry dir")).expect("create registry dir");
    fs::copy(registry_path(&repo, "barrick"), &reg).expect("copy registry");
    let dir = root.join(&profile.corpus_dir);
    fs::create_dir_all(&dir).expect("create scratch corpus dir");
    let first = &profile.sources[0];
    // A real, parseable workbook whose bytes are not the registered ones.
    let bytes = fmine_io::xlsx::build_minimal_xlsx(&[]);
    fs::write(dir.join(&first.file_name), &bytes).expect("write altered file");

    let refusal = run_barrick_m0_assurance(&root).expect_err("an altered file must be refused");
    // The other registered files are missing, so the code is 506 (foundation I/O, exit 5, §30.1); the altered
    // file is still listed under "digest mismatch" with both digests. An altered file alone is 505, exit 8
    // (`live.rs`, `test_live_files_synthetic_all_refuse`).
    assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-506", 5));
    let what = &refusal.what;
    assert!(
        what.contains(&format!("{n} of {n} registered barrick workbooks unusable")),
        "{what}"
    );
    assert!(what.contains(&format!(
        "digest mismatch: {} (registered {} bytes sha256 {}, found {} bytes sha256 ",
        first.file_name,
        first.bytes,
        first.sha256_hex,
        bytes.len()
    )));
    let missing = what.split("; digest mismatch").next().unwrap_or_default();
    for src in &profile.sources[1..] {
        assert!(
            missing.contains(&src.file_name),
            "{} not listed as missing",
            src.file_name
        );
    }
}
