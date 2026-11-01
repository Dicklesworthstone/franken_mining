//! Public-API Claim inspection acceptance, retaining the source-projection goldens and integrity checks.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use fmine_cli::{CliCommand, OutputFormat, execute_command, invoke, parse_args};
use fmine_public::sources::load_issuer_profile;
use fmine_public::{run_public_assurance, run_synthetic_assurance};

const FLAGS: &str = "{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\
                     \"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}";

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn scratch(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fmine-claim-inspection-{tag}-{}",
        std::process::id()
    ))
}

fn synthetic_workspace(tag: &str) -> PathBuf {
    let root = scratch(tag);
    fmine_public::synthetic::write_issuer_workspace(&root).expect("synthetic workspace");
    root
}

fn resolves(command: &str, root: &Path) -> bool {
    match command.strip_prefix("fmine ") {
        Some(rest) => {
            let argv: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
            parse_args(&argv, root).is_ok()
        }
        None => root.join(command).exists(),
    }
}

#[test]
fn test_cli_claim_show_refuses_invalid_id() {
    for action in ["show", "why", "trace"] {
        let env = invoke(&args(&["claim", action, "clm_0001"]), &workspace_root()).0;
        assert_eq!((env.exit_code, env.status), (3, "refused"));
        assert_eq!(env.verb, format!("claim {action}"));
        let json = env.to_json();
        assert!(json.contains("\"code\":\"FMINE-REF-201\""), "{json}");
        assert!(json.contains("invalid public claim id"), "{json}");
    }
}

/// BD-115: hand-written inspection payloads around the independently tested canonical cell JSON.
/// The synthetic source is not registered on disk and remains an assumption with an unbounded color.
#[test]
fn test_claim_show_why_trace_golden() {
    let report = run_synthetic_assurance(&[]).expect("synthetic report");
    let cell = report
        .public_claims
        .cells
        .get("Synthetic_AISC.xlsx:Q1 2030!B5")
        .expect("source cell");
    let id = cell.claim.claim_id.to_string();
    let canonical = cell.to_json();
    assert!(canonical.contains("\"stored\":\"500\",\"displayed\":\"500\""));
    let uu_refusal = "{\"code\":\"FMINE-REF-204\",\"exit_code\":3,\
        \"what\":\"Unbudgeted unknowns: a decision-facing claim carries no unknown-unknown (UU) budget (G-UU, INV-015): \
        claim 'cost of sales applicable to gold production' has epistemic status KnownUnknown(Parametric) but zero UuTerm scenarios\",\
        \"fix\":\"Attach an explicit UU term (an unmodelled budget or a flip margin).\"}";
    let why = format!(
        "{{\"schema\":\"fmine.claim.why.v1\",\"claim\":{canonical},\
         \"interpretation\":\"The source number and its displayed text are preserved; extraction does not verify its truth.\",\
         \"evidence_class\":\"Assumed\",\"epistemic_status\":\"KnownUnknown(Parametric)\",\"knowledge_state\":\"Unknown\",\
         \"representation\":\"Deterministic\",\"support\":\"PeriodAggregate\",\"additivity\":\"ExtensiveAdditive\",\"confidentiality\":\"Public\",\
         \"color\":{{\"status\":\"available\",\"value\":{{\"kind\":\"estimated\",\"estimator\":\"class:assumed\",\
         \"dispersion\":null,\"dispersion_status\":\"unbounded\"}},\"refusal\":null}},\
         \"uu_terms\":[],\"uu_guard\":{{\"status\":\"refused\",\"refusal\":{uu_refusal}}},\
         \"public_export_guard\":{{\"status\":\"passed\",\"refusal\":null}},\
         \"decision_admission\":{{\"scope\":\"Claim color and UU guards only\",\"claim_guards_pass\":false,\
         \"first_refusal\":{uu_refusal},\"decision_context\":null,\"flip_margin\":null}},\
         \"calibration\":{{\"failed\":false,\"assessment\":null}},\
         \"freshness\":{{\"recorded_stale\":false,\"assessment\":\"not_assessed\",\
         \"reason\":\"This source projection has no clock or dependency-invalidation replay.\"}}}}"
    );
    let trace = format!(
        "{{\"schema\":\"fmine.claim.trace.v1\",\"claim\":{canonical},\"scope\":\"source_leaf\",\
         \"complete_for_claim\":true,\"parents\":[],\"unresolved_parents\":[],\
         \"anchors\":[{{\"artifact_path\":\"Synthetic_AISC.xlsx\",\"sha256\":\"{}\",\
         \"locator\":\"Q1 2030!B5\",\"registered_source\":null}}],\"supersedes\":null,\"recorded_stale\":false,\
         \"boundary\":\"Source cells and their registered interpretation are the leaves of this public projection; no upstream measurement history or dependency replay is asserted.\"}}",
        cell.key.reporting_vintage.file_sha256,
    );
    for (action, want) in [("show", canonical), ("why", why), ("trace", trace)] {
        let argv = args(&["claim", action, &id]);
        let (env, format) = invoke(&argv, &workspace_root());
        assert_eq!(
            (env.status, env.exit_code, format),
            ("ok", 0, OutputFormat::Json)
        );
        assert_eq!(env.verb, format!("claim {action}"));
        assert_eq!(env.payload_json, want, "{action}");
        assert_eq!(env.to_json().matches(FLAGS).count(), 1);
        assert_eq!(env.certificate, None);
        assert!(
            env.next_actions
                .iter()
                .all(|a| resolves(&a.command, &workspace_root()))
        );
        assert!(
            env.next_actions
                .iter()
                .any(|a| a.command == "fmine assure public --synthetic")
        );
        assert_eq!(invoke(&argv, &workspace_root()).0.to_json(), env.to_json());
    }
}

#[test]
fn test_claim_why_uses_recorded_guards_and_trace_keeps_unresolved_parents() {
    let report = run_synthetic_assurance(&[]).expect("synthetic report");
    let mut cell = report.public_claims.cells["Synthetic_AISC.xlsx:Q1 2030!B5"].clone();
    cell.claim.uu_terms.push(fmine_claim::UuTerm {
        scenario_id: "synthetic-downside".into(),
        description: "Invented test scenario".into(),
        downside_exposure_usd_m: 2.5,
        trigger_indicator: "synthetic trigger".into(),
    });
    let why = fmine_public::key::inspect::why_json(&cell);
    assert!(why.contains("\"claim_guards_pass\":true"));
    assert!(why.contains("\"downside_exposure_usd_m\":2.5"));
    assert!(why.contains("synthetic-downside"));
    assert!(why.contains("\"decision_context\":null"));
    cell.claim.calibration_failed = true;
    cell.claim.stale = true;
    let why = fmine_public::key::inspect::why_json(&cell);
    assert!(why.contains("FMINE-REF-205"));
    assert!(why.contains("\"claim_guards_pass\":false"));
    assert!(why.contains("\"recorded_stale\":true"));
    assert!(why.contains("\"assessment\":\"not_assessed\""));
    let parent = fmine_core::id::ObjectId::from_canonical_bytes(
        fmine_core::id::PrefixKind::Claim,
        b"unresolved-synthetic-parent",
    );
    cell.claim.parents.push(parent);
    let trace = fmine_public::key::inspect::trace_json(
        &cell,
        &fmine_public::synthetic::synthetic_profile(),
    );
    assert!(trace.contains("\"scope\":\"partial_provenance\",\"complete_for_claim\":false"));
    assert!(trace.contains(&format!("\"unresolved_parents\":[\"{parent}\"]")));
}

#[test]
fn test_claim_inspection_absent_id_is_not_replaced_by_a_synthetic_claim() {
    let missing = fmine_core::id::ObjectId::from_canonical_bytes(
        fmine_core::id::PrefixKind::Claim,
        b"absent-public-claim",
    )
    .to_string();
    let ws = scratch("claim-absent");
    for action in ["show", "why", "trace"] {
        let env = invoke(&args(&["claim", action, &missing]), &ws).0;
        assert_eq!((env.status, env.exit_code), ("refused", 3));
        assert_eq!(env.refusal.expect("refusal").code, "FMINE-REF-201");
        assert_eq!(env.payload_json, "null");
    }
}

#[test]
fn test_cli_claim_show_resolves_aisc_and_refuses_altered_source() {
    let ws = synthetic_workspace("claim-show-aisc");
    let profile = load_issuer_profile(&ws, "synthetic").expect("registry");
    let report = run_public_assurance(&profile, &ws).expect("report");
    let source = profile
        .sources
        .iter()
        .find(|s| s.kind == fmine_public::sources::SourceKind::AiscReconciliation)
        .expect("AISC source");
    let claim = report
        .public_claims
        .cells
        .values()
        .find(|c| c.locator.starts_with(&format!("{}:", source.file_name)))
        .expect("emitted AISC claim");
    let id = claim.claim.claim_id.to_string();
    for action in ["show", "why", "trace"] {
        let env = invoke(&args(&["claim", action, &id]), &ws).0;
        assert_eq!((env.status, env.exit_code), ("ok", 0), "{}", env.to_json());
        if action == "show" {
            assert_eq!(env.payload_json, claim.to_json());
        }
        if action == "why" {
            assert!(env.payload_json.contains("\"claim_guards_pass\":false"));
            assert!(env.payload_json.contains("FMINE-REF-204"));
        }
        if action == "trace" {
            assert!(env.payload_json.contains("\"scope\":\"source_leaf\""));
            assert!(
                env.payload_json
                    .contains(&format!("\"url\":\"{}\"", source.url))
            );
            assert!(!env.payload_json.contains("\"registered_source\":null"));
        }
        assert!(env.payload_json.contains("\"evidence\":\"synthetic\""));
        assert!(env.to_json().contains(FLAGS));
        assert!(env.next_actions.iter().all(|a| resolves(&a.command, &ws)));
    }

    let path = ws.join(&profile.corpus_dir).join(&source.file_name);
    let mut bytes = std::fs::read(&path).expect("source bytes");
    bytes.push(0);
    std::fs::write(&path, bytes).expect("alter the registered source");
    for action in ["show", "why", "trace"] {
        let refused = invoke(&args(&["claim", action, &id]), &ws).0;
        assert_eq!((refused.status, refused.exit_code), ("refused", 8));
        assert_eq!(refused.refusal.expect("refusal").code, "FMINE-REF-505");
        assert_eq!(refused.payload_json, "null");
    }
}

#[test]
#[ignore = "needs the registered Barrick workbooks"]
fn test_cli_claim_show_resolves_public_source_claim() {
    let root = workspace_root();
    let profile = load_issuer_profile(&root, "barrick").expect("registry");
    let claims = fmine_public::key::mine_stats::load_claims(&profile, &root).expect("claims");
    let first = claims
        .claims
        .iter()
        .find(|c| c.locator == "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23")
        .expect("E474 first");
    let env = execute_command(
        &CliCommand::ClaimShow {
            id: first.claim.claim_id.to_string(),
            issuer: None,
            through: None,
            synthetic: false,
        },
        &root,
    );
    assert_eq!((env.status, env.exit_code), ("ok", 0));
    assert_eq!(env.payload_json, first.to_json());
    assert!(env.to_json().contains("1479"));
    assert!(env.to_json().contains(FLAGS));
    let id = first.claim.claim_id.to_string();
    let why = invoke(&args(&["claim", "why", &id]), &root).0;
    assert_eq!((why.status, why.exit_code), ("ok", 0));
    assert!(why.payload_json.contains("Assertion { source: Issuer }"));
    assert!(why.payload_json.contains("\"knowledge_state\":\"Known\""));
    assert!(why.payload_json.contains("FMINE-REF-205"));
    assert!(!why.payload_json.contains("FMINE-REF-204"));
    assert!(why.payload_json.contains("\"claim_guards_pass\":false"));
    let trace = invoke(&args(&["claim", "trace", &id]), &root).0;
    assert_eq!((trace.status, trace.exit_code), ("ok", 0));
    assert!(trace.payload_json.contains("\"complete_for_claim\":true"));
    assert!(
        trace
            .payload_json
            .contains(&first.key.reporting_vintage.file_sha256)
    );
    let source = profile
        .sources
        .iter()
        .find(|s| s.file_name == "Barrick_Q1_2024_Mine_Stats.xlsx")
        .expect("registered source");
    assert!(trace.payload_json.contains(&source.url));
    assert!(trace.payload_json.contains("\"locator\":\"QTD!G23\""));
}

#[test]
fn test_claim_show_reads_reported_claims_and_refuses_invented_ids() {
    let report = run_synthetic_assurance(&[]).expect("synthetic report");
    for is_numeric in [false, true] {
        let cell = report
            .public_claims
            .cells
            .values()
            .find(|c| {
                matches!(c.claim.value, fmine_public::key::PublicValue::Number { .. }) == is_numeric
            })
            .expect("numeric and placeholder Claims");
        let id = cell.claim.claim_id.to_string();
        for action in ["show", "why", "trace"] {
            let query = args(&["claim", action, &id, "--synthetic"]);
            let (env, _) = invoke(&query, &workspace_root());
            assert_eq!((env.exit_code, env.status), (0, "ok"), "{}", env.to_json());
            assert!(env.payload_json.contains(&cell.to_json()));
            assert!(env.payload_json.contains("\"lane\":\"synthetic\""));
            assert_eq!(
                env.certificate,
                Some(report.ledger_anchor.state_root.to_hex())
            );
            assert!(env.payload_json.contains(
                &fmine_public::key::claims_digest(report.public_claims.cells.values()).to_hex()
            ));
            if action != "show" {
                assert!(env.payload_json.contains(&format!("\"{action}\":{{")));
            }
            assert!(env.to_json().contains(FLAGS));
            assert_eq!(
                invoke(&query, &workspace_root()).0,
                env,
                "the same source snapshot reads byte-identically"
            );
            assert!(
                env.next_actions
                    .iter()
                    .all(|a| a.command.ends_with("--synthetic"))
            );
        }
    }
    for action in ["show", "why", "trace"] {
        let (unknown, _) = invoke(
            &args(&["claim", action, "clm_invented", "--synthetic"]),
            &workspace_root(),
        );
        assert_eq!((unknown.exit_code, unknown.status), (3, "refused"));
        assert_eq!(unknown.payload_json, "null");
        assert!(
            unknown
                .refusal
                .expect("missing id refusal")
                .what
                .contains("absent")
        );
    }
}

#[test]
#[ignore = "needs the registered Barrick workbooks"]
fn test_claim_show_reads_real_mine_stats_and_aisc_claims() {
    let root = workspace_root();
    let profile = load_issuer_profile(&root, "barrick").expect("profile");
    let report = run_public_assurance(&profile, &root).expect("verified public report");
    for is_mine_stats in [false, true] {
        let cell = report
            .public_claims
            .cells
            .values()
            .find(|c| {
                (c.key.line.label.starts_with("site:") || c.key.line.label.starts_with("summary:"))
                    == is_mine_stats
            })
            .expect("both real-source adapters emit Claims");
        let id = cell.claim.claim_id.to_string();
        let (env, _) = invoke(&args(&["claim", "show", &id, "--issuer", "barrick"]), &root);
        assert_eq!(env.exit_code, 0, "{}", env.to_json());
        assert!(env.payload_json.contains(&cell.to_json()));
        assert!(env.payload_json.contains("\"lane\":\"public\""));
        assert_eq!(
            env.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
    }
}

#[test]
fn test_claim_show_rechecks_registered_source_bytes() {
    let ws = synthetic_workspace("claim-source-integrity");
    let profile = load_issuer_profile(&ws, "synthetic").expect("registered synthetic source");
    let bounded = profile.clone().through("2030Q1").expect("bound");
    let report = run_public_assurance(&bounded, &ws).expect("verified report");
    let cell = report
        .public_claims
        .cells
        .values()
        .next()
        .expect("public claim");
    let id = cell.claim.claim_id.to_string();
    let later = profile
        .sources
        .iter()
        .find(|s| s.quarter == "2030Q2")
        .expect("later source");
    std::fs::write(
        ws.join(&profile.corpus_dir).join(&later.file_name),
        b"altered later source",
    )
    .expect("alter later scratch fixture");
    for action in ["show", "why", "trace"] {
        let query = args(&[
            "claim",
            action,
            &id,
            "--issuer",
            "synthetic",
            "--through",
            "2030Q1",
        ]);
        let (read, _) = invoke(&query, &ws);
        assert_eq!(read.exit_code, 0, "{}", read.to_json());
        assert!(read.payload_json.contains(&cell.to_json()));
        assert_eq!(
            read.certificate,
            Some(report.ledger_anchor.state_root.to_hex())
        );
        assert!(
            read.next_actions
                .iter()
                .all(|a| a.command.ends_with("--issuer synthetic --through 2030Q1"))
        );
        assert!(read.next_actions.iter().all(|a| resolves(&a.command, &ws)));
        let unbounded = invoke(&args(&["claim", action, &id, "--issuer", "synthetic"]), &ws).0;
        assert_eq!(
            unbounded.refusal.expect("later source mismatch").code,
            "FMINE-REF-505"
        );
        let wrong_lane = invoke(&args(&["claim", action, &id, "--synthetic"]), &ws).0;
        assert_eq!(
            wrong_lane.refusal.expect("id absent from fixture").code,
            "FMINE-REF-201"
        );
    }
    let path = ws
        .join(&profile.corpus_dir)
        .join(&bounded.sources[0].file_name);
    std::fs::write(path, b"altered registered source").expect("alter scratch fixture");
    for action in ["show", "why", "trace"] {
        let query = args(&[
            "claim",
            action,
            &id,
            "--issuer",
            "synthetic",
            "--through",
            "2030Q1",
        ]);
        let (refused, _) = invoke(&query, &ws);
        assert_eq!((refused.exit_code, refused.status), (8, "refused"));
        assert_eq!(
            refused.refusal.expect("digest refusal").code,
            "FMINE-REF-505"
        );
        assert_eq!(refused.payload_json, "null");
    }
}
