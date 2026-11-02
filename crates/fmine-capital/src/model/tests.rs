//! Source-bound entry-point tests; policy arithmetic has separate independent known answers.
//! Determinism: Exact (original source bytes and exact arithmetic outcomes remain separately bound).

use super::*;

const STAGED: &str = include_str!("../../../../tests/capital/recourse.toml");
const STATIC: &str = include_str!("../../../../tests/capital/cash_schedule.toml");

fn source(text: &str) -> SourceAnchor {
    SourceAnchor::new(
        "synthetic/capital.toml",
        sha256(text.as_bytes()).0,
        "declared static capital program",
    )
}

#[test]
fn static_evaluation_delegates_without_changing_any_output() {
    let original = crate::evaluate(&parse(STATIC, source(STATIC)).unwrap()).unwrap();
    let evaluated = evaluate(STATIC, source(STATIC)).unwrap();
    assert!(matches!(&evaluated, EvaluatedModel::Static(_)));
    assert_eq!(evaluated.mode(), "declared_static_capital_comparison");
    assert_eq!(evaluated.to_json(), original.to_json());
    assert_eq!(evaluated.to_markdown(), original.to_markdown());
    assert_eq!(evaluated.no_claim(), original.no_claim());
}

#[test]
fn static_parser_refuses_every_recourse_table() {
    assert_eq!(
        parse(STAGED, source(STAGED)).unwrap_err().code,
        "FMINE-REF-506"
    );
    for row in [
        "[[recourse]]\ninformation_candidate = \"bridge\"\nobservation_date = \"2030-07-01\"\ninformation_basis = \"Declared signal\"\n",
        "[[candidate_stage]]\ncandidate = \"bridge\"\nstage = \"initial\"\n",
        "[[state_observation]]\nstate = \"base\"\nobservation = \"same\"\n",
    ] {
        let text = format!("{STATIC}\n{row}");
        let refusal = parse(&text, source(&text)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("unknown table"));
    }
}

#[test]
fn staged_source_hash_binds_the_original_file_including_stage_rows_and_comments() {
    let mut wrong = source(STAGED);
    wrong.artifact_sha256 = sha256(STATIC.as_bytes()).0;
    assert_eq!(evaluate(STAGED, wrong).unwrap_err().code, "FMINE-REF-505");

    let changed = format!("{STAGED}\n# unchanged model, newly annotated original source\n");
    assert_eq!(
        evaluate(&changed, source(STAGED)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let original = evaluate(STAGED, source(STAGED)).unwrap();
    let annotated = evaluate(&changed, source(&changed)).unwrap();
    let (EvaluatedModel::Recourse(original), EvaluatedModel::Recourse(annotated)) =
        (original, annotated)
    else {
        panic!("explicit recourse declarations must evaluate as recourse");
    };
    assert_eq!(original.summary(), annotated.summary());
    assert_ne!(original.digest(), annotated.digest());
    assert_ne!(
        original.inputs().information.claim_id,
        annotated.inputs().information.claim_id
    );
    assert_ne!(
        original.static_program().digest(),
        annotated.static_program().digest()
    );
}

#[test]
fn staged_information_claim_retains_all_row_sources_and_assumption_metadata() {
    let evaluated = evaluate(STAGED, source(STAGED)).unwrap();
    assert_eq!(evaluated.mode(), "declared_two_stage_capital_comparison");
    assert!(
        evaluated
            .to_json()
            .contains("\"schema\":\"fmine.capital-recourse.v1\"")
    );
    assert!(
        evaluated
            .to_markdown()
            .contains("# Two-stage capital policy comparison")
    );
    assert_eq!(evaluated.no_claim(), crate::recourse::NO_CLAIM);
    let EvaluatedModel::Recourse(report) = evaluated else {
        panic!("staged source must retain its separate evaluated decision object");
    };
    let information = &report.inputs().information;
    assert_eq!(
        information.metric_key,
        "capital.invented_two_stage_equity_and_cash.information"
    );
    assert_eq!(information.evidence, EvidenceClass::Assumed);
    assert_eq!(information.state, KnowledgeState::Estimated);
    assert_eq!(information.additivity, AdditivityClass::NonAdditive);
    assert_eq!(information.confidentiality, Confidentiality::internal());
    assert_eq!(
        information.uu_terms,
        report.inputs().program.policy.uu_terms
    );
    assert!(report.claim().parents.contains(&information.claim_id));
    for suffix in [
        "recourse information gate, observation date and basis",
        "candidate stage study",
        "candidate stage issue",
        "candidate stage project",
        "state observation good/good",
        "state observation bad/bad",
        "model_key",
        "currency",
        "valuation_basis",
        "UU study_accuracy_and_execution",
    ] {
        assert!(
            information
                .anchors
                .iter()
                .any(|anchor| anchor.locator.ends_with(&format!(": {suffix}"))),
            "missing source anchor: {suffix}"
        );
    }
    for anchor in &information.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(STAGED.as_bytes()).0);
        assert_eq!(anchor.artifact_path, "synthetic/capital.toml");
        assert!(
            anchor
                .locator
                .starts_with("declared static capital program; line ")
        );
    }
    for anchor in &report.static_program().claim().anchors {
        assert_eq!(anchor.artifact_sha256, sha256(STAGED.as_bytes()).0);
    }
}

#[test]
fn staged_parser_rejects_duplicate_unknown_and_invalid_declarations() {
    let declaration = "\n[[recourse]]\ninformation_candidate = \"study\"\nobservation_date = \"2030-07-01\"\ninformation_basis = \"Another declaration\"\n";
    let invalid = [
        format!("{STAGED}{declaration}"),
        format!("{STAGED}\n[[candidate_stage]]\ncandidate = \"study\"\nstage = \"initial\"\n"),
        format!("{STAGED}\n[[state_observation]]\nstate = \"good\"\nobservation = \"good\"\n"),
        STAGED.replace(
            "information_candidate = \"study\"",
            "information_candidate = \"study\"\nextra = \"undeclared\"",
        ),
        STAGED.replace("stage = \"initial\"", "stage = \"adaptive\""),
        STAGED.replace(
            "[[candidate_stage]]\ncandidate = \"project\"",
            "[[candidate_stage]]\ncandidate = \"unknown\"",
        ),
        STAGED.replace(
            "[[state_observation]]\nstate = \"bad\"",
            "[[state_observation]]\nstate = \"unknown\"",
        ),
        format!("{STAGED}\n[[unregistered_stage]]\nvalue = \"unknown\"\n"),
    ];
    for text in invalid {
        assert_ne!(text, STAGED, "mutation must change its source");
        assert_eq!(
            evaluate(&text, source(&text)).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
}

#[test]
fn staged_parser_requires_explicit_complete_stage_and_observation_matrices() {
    for row in [
        "[[candidate_stage]]\ncandidate = \"bridge\"\nstage = \"initial\"\n",
        "[[state_observation]]\nstate = \"base\"\nobservation = \"same\"\n",
    ] {
        let text = format!("{STATIC}\n{row}");
        let refusal = evaluate(&text, source(&text)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("explicit recourse declaration"));
    }
    for row in [
        "[[candidate_stage]]\ncandidate = \"project\"\nstage = \"recourse\"\n",
        "[[state_observation]]\nstate = \"bad\"\nobservation = \"bad\"\n",
    ] {
        let text = STAGED.replace(row, "");
        assert_ne!(text, STAGED, "mutation must remove a declared row");
        assert_eq!(
            evaluate(&text, source(&text)).unwrap_err().code,
            "FMINE-REF-201"
        );
    }
}
