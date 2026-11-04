//! DESIGN (BD-229): strict, original-source-bound bootstrap inputs in both capital modes.
//! Determinism: Exact (seeded results and evidence identity are checked separately).

use super::*;

const DECLARATION: &str = "\n[[selection_stability]]\nmethod = \"weighted_multinomial\"\nseed = \"229\"\nreplicates = \"200\"\ndraws_per_replicate = \"2\"\nsampling_basis = \"Synthetic finite-law sensitivity; two draws are a declared experiment size\"\n";

fn static_source() -> String {
    format!("{STATIC}{DECLARATION}")
}

#[test]
fn bootstrap_declaration_keeps_original_sources_and_assumption_metadata() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let claim = inputs.selection_stability.as_ref().unwrap();
    assert_eq!(claim.metric_key, "capital.selection-stability/1");
    assert_eq!(claim.value.model_key, inputs.policy.value.model_key);
    assert_eq!(claim.value.method, crate::stability::METHOD);
    assert_eq!(claim.value.seed, 229);
    assert_eq!(claim.value.replicates, 200);
    assert_eq!(claim.value.draws_per_replicate, 2);
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert_eq!(claim.additivity, AdditivityClass::NonAdditive);
    assert_eq!(claim.confidentiality, Confidentiality::internal());
    assert_eq!(claim.uu_terms, inputs.policy.uu_terms);
    assert!(claim.anchors.iter().any(|anchor| {
        anchor
            .locator
            .ends_with(": selection stability method, seed, replicate count, draw count and basis")
    }));
    assert!(
        claim
            .anchors
            .iter()
            .any(|anchor| anchor.locator.ends_with(": model_key"))
    );
    for anchor in &claim.anchors {
        assert_eq!(anchor.artifact_path, "synthetic/capital.toml");
        assert_eq!(anchor.artifact_sha256, sha256(text.as_bytes()).0);
        assert!(
            anchor
                .locator
                .starts_with("declared static capital program; line ")
        );
    }
    let report = crate::evaluate(&inputs).unwrap();
    assert!(report.claim().parents.contains(&claim.claim_id));
    assert!(report.summary().selection_stability.is_some());
    let dispatched = evaluate(&text, source(&text)).unwrap();
    assert_eq!(report.to_json(), dispatched.to_json());
    assert_eq!(report.to_markdown(), dispatched.to_markdown());
    let without = crate::evaluate(&parse(STATIC, source(STATIC)).unwrap()).unwrap();
    assert_eq!(report.summary().choices, without.summary().choices);
    assert_eq!(
        report.summary().feasible_winners,
        without.summary().feasible_winners
    );
    assert!(without.inputs().selection_stability.is_none());
    assert!(without.summary().selection_stability.is_none());
}

#[test]
fn staged_bootstrap_uses_the_same_source_bound_design_for_policies_and_static_comparison() {
    let text = format!("{STAGED}{DECLARATION}");
    assert_eq!(
        parse(&text, source(&text)).unwrap_err().code,
        "FMINE-REF-506"
    );
    let EvaluatedModel::Recourse(report) = evaluate(&text, source(&text)).unwrap() else {
        panic!("staged declarations must retain the two-stage object");
    };
    let claim = report
        .inputs()
        .program
        .selection_stability
        .as_ref()
        .unwrap();
    assert_eq!(
        claim,
        report
            .static_program()
            .inputs()
            .selection_stability
            .as_ref()
            .unwrap()
    );
    assert!(report.summary().selection_stability.is_some());
    assert!(
        report
            .static_program()
            .summary()
            .selection_stability
            .is_some()
    );
    assert!(report.claim().parents.contains(&claim.claim_id));
    assert!(report.to_json().contains("\"execution_authorized\":false"));
    assert!(
        report
            .to_json()
            .contains("\"reserve_disclosure_claim\":false")
    );
}

#[test]
fn bootstrap_controls_and_original_source_bytes_change_the_receipt() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let original = crate::evaluate(&inputs).unwrap();
    let claim = inputs.selection_stability.as_ref().unwrap();
    for (before, after) in [
        ("seed = \"229\"", "seed = \"230\""),
        ("replicates = \"200\"", "replicates = \"201\""),
        ("draws_per_replicate = \"2\"", "draws_per_replicate = \"3\""),
        (
            "two draws are a declared experiment size",
            "a different declared sampling rationale",
        ),
    ] {
        let changed = text.replace(before, after);
        assert_ne!(changed, text);
        let changed_inputs = parse(&changed, source(&changed)).unwrap();
        let changed_claim = changed_inputs.selection_stability.as_ref().unwrap();
        assert_ne!(
            changed_claim.value.canonical_tag(),
            claim.value.canonical_tag()
        );
        assert_ne!(changed_claim.claim_id, claim.claim_id);
        assert_ne!(
            crate::evaluate(&changed_inputs).unwrap().digest(),
            original.digest()
        );
    }
    let annotated = format!("{text}\n# Another annotation on the same declared experiment\n");
    assert_eq!(
        parse(&annotated, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let annotated_inputs = parse(&annotated, source(&annotated)).unwrap();
    let annotated_report = crate::evaluate(&annotated_inputs).unwrap();
    assert_eq!(annotated_report.summary(), original.summary());
    assert_ne!(annotated_report.digest(), original.digest());

    let at = STATIC.find("[[state]]").unwrap();
    let reordered = format!("{}{DECLARATION}{}", &STATIC[..at], &STATIC[at..]);
    let reordered_inputs = parse(&reordered, source(&reordered)).unwrap();
    assert_eq!(
        reordered_inputs.selection_stability.as_ref().unwrap().value,
        claim.value
    );
    assert_eq!(
        crate::evaluate(&reordered_inputs).unwrap().summary(),
        original.summary()
    );
}

#[test]
fn bootstrap_parser_refuses_duplicate_unknown_missing_and_noninteger_controls() {
    let text = static_source();
    let invalid = [
        format!("{text}{DECLARATION}"),
        format!("{text}unexpected = \"1\"\n"),
        text.replace("method = \"weighted_multinomial\"", "method = \"uniform\""),
        text.replace("seed = \"229\"\n", ""),
        text.replace("seed = \"229\"", "seed = 229"),
        text.replace("seed = \"229\"", "seed = \"-1\""),
        text.replace("seed = \"229\"", "seed = \"+1\""),
        text.replace("seed = \"229\"", "seed = \"1_000\""),
        text.replace("seed = \"229\"", "seed = \"18446744073709551616\""),
        text.replace("replicates = \"200\"", "replicates = \"199\""),
        text.replace("replicates = \"200\"", "replicates = \"2001\""),
        text.replace("replicates = \"200\"", "replicates = \"200.0\""),
        text.replace("draws_per_replicate = \"2\"", "draws_per_replicate = \"0\""),
        text.replace(
            "draws_per_replicate = \"2\"",
            "draws_per_replicate = \"4097\"",
        ),
        text.replace(
            "draws_per_replicate = \"2\"",
            "draws_per_replicate = \"1/2\"",
        ),
        text.replace(
            "Synthetic finite-law sensitivity; two draws are a declared experiment size",
            "",
        ),
    ];
    for invalid in invalid {
        let refusal = parse(&invalid, source(&invalid)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506", "{}", refusal.what);
    }
    for (before, after) in [
        ("seed = \"229\"", "seed = \"0\""),
        ("seed = \"229\"", "seed = \"18446744073709551615\""),
        ("replicates = \"200\"", "replicates = \"2000\""),
        ("draws_per_replicate = \"2\"", "draws_per_replicate = \"1\""),
        (
            "draws_per_replicate = \"2\"",
            "draws_per_replicate = \"4096\"",
        ),
    ] {
        let boundary = text.replace(before, after);
        parse(&boundary, source(&boundary)).unwrap();
    }
}
