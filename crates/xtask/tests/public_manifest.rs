//! BD-322 (bd-gwf1): a run's input manifest is its registry. The report's sources are the registered files, in
//! registry order, with the registered sha256 and byte count, and every registered file contributes located
//! tables: AISC workbooks through `locate_table`, Mine Stats workbooks through the label-anchored reader.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use fmine_core::hash::Digest32;
    use fmine_public::sources::{IssuerProfile, SourceKind};
    use fmine_public::{
        PublicAssuranceReport, load_issuer_profile, run_public_assurance, synthetic,
    };

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// `(file, kind, quarter, bytes, sha256)` of the registry and of the report, which must be equal row by row.
    fn manifest_rows(
        profile: &IssuerProfile,
        r: &PublicAssuranceReport,
    ) -> (Vec<String>, Vec<String>) {
        let registry = profile
            .sources
            .iter()
            .map(|s| {
                format!(
                    "{} {:?} {} {} {}",
                    s.file_name, s.kind, s.quarter, s.bytes, s.sha256_hex
                )
            })
            .collect();
        let report = r
            .sources
            .iter()
            .map(|s| {
                format!(
                    "{} {:?} {} {} {}",
                    s.file_name,
                    s.kind,
                    s.quarter,
                    s.bytes,
                    Digest32(s.workbook.sha256).to_hex()
                )
            })
            .collect();
        (registry, report)
    }

    /// Every AISC workbook gives at least one identity instance and no table refusal; every Mine Stats workbook
    /// has a label-anchored read with at least one block and one value. Returns the files that do not.
    fn files_without_tables(r: &PublicAssuranceReport) -> Vec<String> {
        let mut bad = Vec::new();
        for s in &r.sources {
            let ok = match s.kind {
                SourceKind::AiscReconciliation => {
                    r.instances.iter().any(|i| i.file == s.file_name)
                        && r.table_refusals.iter().all(|t| t.file != s.file_name)
                }
                SourceKind::MineStats => r.mine_stats.iter().any(|(f, read)| {
                    *f == s.file_name && read.blocks > 0 && !read.values.is_empty()
                }),
            };
            if !ok {
                bad.push(format!("{} ({:?})", s.file_name, s.kind));
            }
        }
        bad
    }

    fn run(profile: &IssuerProfile, root: &Path) -> PublicAssuranceReport {
        run_public_assurance(profile, root).expect("registered files present and unaltered")
    }

    /// Default suite, synthetic lane: the generated registry and its scratch corpus go through the same registry
    /// and digest path as the real lane. The later Mine Stats vintage holds a title only, so it is listed in the
    /// manifest but has no tables, and the check names exactly that file.
    #[test]
    fn test_synthetic_input_manifest_equals_registry() {
        let ws = std::env::temp_dir().join(format!("fmine-manifest-{}", std::process::id()));
        synthetic::write_issuer_workspace(&ws).expect("write workspace");
        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        let r = run(&profile, &ws);
        let (registry, report) = manifest_rows(&profile, &r);
        assert_eq!(registry.len(), 3);
        assert_eq!(report, registry);
        // The synthetic Mine Stats workbooks hold a reconstruction component (read by locator) and a title, not a
        // label-read table, so contribution is checked here for the AISC workbook and on the real lane for both.
        let missing = files_without_tables(&r);
        assert!(
            missing.iter().all(|f| f.ends_with("(MineStats)")),
            "{missing:?}"
        );

        // Planted: a registry listing the same files in another order is still the manifest, row for row.
        let mut reordered = profile.clone();
        reordered.sources.reverse();
        let r = run(&reordered, &ws);
        let (registry, report) = manifest_rows(&reordered, &r);
        assert_eq!(report, registry);
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_input_manifest_equals_registry_and_every_file_contributes() {
        let root = workspace_root();
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let r = run(&profile, &root);
        let (registry, report) = manifest_rows(&profile, &r);
        assert_eq!(report, registry);
        let count = |k: SourceKind| r.sources.iter().filter(|s| s.kind == k).count();
        let (aisc, stats) = (
            count(SourceKind::AiscReconciliation),
            count(SourceKind::MineStats),
        );
        assert_eq!((r.sources.len(), aisc, stats), (18, 8, 10));
        assert_eq!(files_without_tables(&r), Vec::<String>::new());
        println!(
            "{{\"files\":{},\"aisc\":{},\"mine_stats\":{},\"aisc_tables\":{},\"instances\":{},\"mine_stats_reads\":{}}}",
            r.sources.len(),
            aisc,
            stats,
            r.aisc_tables,
            r.instances.len(),
            r.mine_stats.len()
        );
    }
}
