//! Live dependency on the source registry: an altered registered file is refused with both digests, never read.
//! The registry itself is read from `registries/sources/barrick.toml`, copied into a scratch workspace.

use std::fs;
use std::path::PathBuf;

use fmine_public::run_barrick_m0_assurance;
use fmine_public::sources::{load_issuer_profile, registry_path};

#[test]
fn altered_registered_file_is_refused_with_both_digests() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
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
    assert_eq!((refusal.code, refusal.exit_code), ("FMINE-REF-502", 8));
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
