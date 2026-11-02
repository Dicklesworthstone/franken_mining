// Cross-crate source registry, historical root and canonical anchor contract.

fn assert_source_fields_affect_frame(bytes: impl Fn(&IdentityState) -> Vec<u8>) {
    // D13 100 → 101 keeps D16 (iv) but changes q in its fields and its operand.
    let d16 = |edits: &[(String, MinimalCell)]| {
        run_synthetic_assurance(edits)
            .expect("synthetic run")
            .instances
            .into_iter()
            .find(|i| i.cell == "D16" && i.family == IdentityFamily::AiscPerOunce)
            .expect("D16")
    };
    let (a, b) = (
        d16(&[]),
        d16(&[("D13".to_string(), MinimalCell::Number(101.0))]),
    );
    assert_eq!((a.state.code(), b.state.code()), ("iv", "iv"));
    assert_ne!(bytes(&a.state), bytes(&b.state));
}

/// BD-339.S1: the expected-roots file parses and every row is well formed. Each anchor is the one the run
/// derives from its root (18 sources, public lane), so a typo in either is caught without the corpus.
#[test]
fn test_expected_roots_file_is_well_formed() {
    let rows =
        sources::load_expected_roots(&sources::expected_roots_path(&workspace_root(), "barrick"))
            .expect("expected roots");
    assert!(rows.len() >= 5, "rows are append-only");
    let hex64 = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    let registry = load_issuer_profile(&workspace_root(), "barrick")
        .expect("registry")
        .through("2026Q2")
        .expect("bound");
    for row in &rows {
        let at = format!("row of {} (line {})", row.commit, row.line);
        assert!(
            row.commit.len() >= 7 && row.commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "{at}"
        );
        for h in [
            &row.registry_epoch,
            &row.state_root,
            &row.census_digest_of_digests,
        ] {
            assert!(hex64(h), "{at}: {h}");
        }
        assert!(!row.reason.is_empty() && !row.evidence.is_empty(), "{at}");
        assert_eq!(row.totals.iter().sum::<u64>(), 2595, "{at}");
        let mut root = [0_u8; 32];
        for (i, byte) in root.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&row.state_root[2 * i..2 * i + 2], 16).expect("hex");
        }
        let root = Digest32(root);
        assert_eq!(
            ledger_anchor("barrick", Lane::Public, registry.sources.len(), root)
                .anchor_id()
                .to_string(),
            row.anchor_id,
            "{at}"
        );
    }
    // E822's row, resolved by re-running commit 748da79 on the corpus.
    assert_eq!(
        (rows[0].commit.as_str(), rows[0].totals),
        ("748da79", [2120, 3, 4, 17, 391, 60])
    );
    let roots: std::collections::BTreeSet<&str> =
        rows.iter().map(|r| r.state_root.as_str()).collect();
    assert_eq!(roots.len(), rows.len(), "each row records a new root");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}
