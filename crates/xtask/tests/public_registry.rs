//! Live dependency on the source registry: an altered registered file is refused with both digests, never read.
//! The registry itself is read from `registries/sources/barrick.toml`, copied into a scratch workspace.

use std::fs;
use std::path::PathBuf;

use fmine_public::run_barrick_m0_assurance;
use fmine_public::sources::{load_issuer_profile, registry_path};
use fmine_public::{Lane, run_public_assurance, sources, synthetic};

#[cfg(test)]
mod source_contract {
    use super::*;
    use fmine_public::aisc::IdentityFamily;
    use fmine_public::sources::{SourceKind, is_through_bound, registered_issuers};

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    // The field-by-field golden test against the former Rust constants passed at commit 997b65e and was removed
    // with them. What still binds the registry: registry-check (xtask), the rows' digests checked on every load,
    // and the real-corpus root pinned in `barrick_real_files_match_the_independent_reader`.

    /// The registry crate's name lists must match this crate's enums, or a checked registry could fail to map.
    #[test]
    fn test_registry_name_lists_match_enums() {
        let kinds: Vec<&str> = SourceKind::ALL.iter().map(|k| k.as_str()).collect();
        assert_eq!(kinds, fmine_registry::sources::SOURCE_KINDS);
        let families: Vec<&str> = IdentityFamily::ALL.iter().map(|f| f.as_str()).collect();
        assert_eq!(families, fmine_registry::sources::IDENTITY_FAMILIES);
    }

    #[test]
    fn test_through_filters_by_quarter() {
        let p = load_issuer_profile(&workspace_root(), "barrick").expect("barrick registry");
        let all = p.sources.len();
        let h1_2025 = p.clone().through("2025Q2").expect("rows remain");
        assert_eq!(h1_2025.through.as_deref(), Some("2025Q2"));
        assert!(h1_2025.sources.len() < all);
        assert!(
            h1_2025
                .sources
                .iter()
                .all(|s| s.quarter.as_str() <= "2025Q2")
        );
        assert!(h1_2025.sources.iter().any(|s| s.quarter == "2025Q2"));
        assert!(
            h1_2025.restatement_notes.is_empty(),
            "E461 must not leak into an earlier cut"
        );
        assert!(h1_2025.restatement_scopes.is_empty());
        let q4_2025 = p.clone().through("2025Q4").expect("E461 release");
        assert!(
            q4_2025
                .restatement_notes
                .iter()
                .any(|note| note.id == "E461")
        );
        assert_eq!(
            q4_2025
                .restatement_scopes
                .iter()
                .filter(|scope| scope.note == "E461")
                .count(),
            18
        );
        // Order is preserved: the filtered rows are a prefix-respecting subsequence of the registry.
        let names: Vec<&str> = h1_2025
            .sources
            .iter()
            .map(|s| s.file_name.as_str())
            .collect();
        let kept: Vec<&str> = p
            .sources
            .iter()
            .filter(|s| s.quarter.as_str() <= "2025Q2")
            .map(|s| s.file_name.as_str())
            .collect();
        assert_eq!(names, kept);
        assert_eq!(p.clone().through("2099Q4").expect("all").sources.len(), all);
        let e = p.through("2023Q4").expect_err("nothing that early");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-201", 3));
        assert!(
            is_through_bound("2026Q2")
                && !is_through_bound("2026Q5")
                && !is_through_bound("2026-Q2")
        );
    }

    #[test]
    fn test_missing_or_malformed_registry_refuses_naming_file_and_line() {
        let dir =
            std::env::temp_dir().join(format!("fmine-public-registry-{}", std::process::id()));
        let sources = dir.join("registries").join("sources");
        std::fs::create_dir_all(&sources).expect("temp registry dir");
        let e = load_issuer_profile(&dir, "nobody").expect_err("no file");
        assert_eq!((e.code, e.exit_code), ("FMINE-REF-506", 5));
        assert!(e.what.contains("nobody.toml"), "{}", e.what);
        let text =
            std::fs::read_to_string(registry_path(&workspace_root(), "barrick")).expect("read");
        let broken = text.replacen("bytes = 43_668", "bytes = 43.668", 1);
        let line = broken
            .lines()
            .position(|l| l == "bytes = 43.668")
            .map(|i| i + 1)
            .expect("edited");
        std::fs::write(sources.join("barrick.toml"), broken).expect("write");
        let e = load_issuer_profile(&dir, "barrick").expect_err("float refused");
        assert!(
            e.what.contains(&format!("line {line}: ")) && e.what.contains("floats"),
            "{}",
            e.what
        );
        assert!(load_issuer_profile(&dir, "../etc").is_err());
        assert_eq!(
            registered_issuers(&workspace_root()),
            vec!["barrick".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

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
