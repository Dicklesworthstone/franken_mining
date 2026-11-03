//! Source-bound entry-point tests; policy arithmetic has separate independent known answers.
//! Determinism: Exact (original source bytes and exact arithmetic outcomes remain separately bound).

use super::*;

mod debt;
mod entity_valuation;

const STAGED: &str = include_str!("../../../../tests/capital/recourse.toml");
const STATIC: &str = include_str!("../../../../tests/capital/cash_schedule.toml");
const ENTITY: &str = include_str!("../../../../tests/capital/entity_funding.toml");

fn entity_static_source() -> String {
    let staged = ENTITY.find("[[recourse]]").unwrap();
    let unknowns = staged + ENTITY[staged..].find("[[uu]]").unwrap();
    format!("{}{}", &ENTITY[..staged], &ENTITY[unknowns..])
}

fn first_table_block(text: &str, table: &str) -> String {
    let document = parse_subset(text).unwrap();
    let index = document
        .rows
        .iter()
        .position(|row| row.table == table)
        .unwrap();
    let lines: Vec<_> = text.lines().collect();
    let begin = document.rows[index].line - 1;
    let end = document
        .rows
        .get(index + 1)
        .map_or(lines.len(), |row| row.line - 1);
    format!("{}\n", lines[begin..end].join("\n"))
}

fn entity_with_transfer() -> String {
    format!(
        "{}\n[[entity_transfer]]\nid = \"call\"\nfrom_entity = \"parent\"\nto_entity = \"jv\"\nperiod = \"second\"\ncandidate = \"project\"\nbasis = \"Declared candidate capital call\"\n\n[[entity_transfer_amount]]\ntransfer = \"call\"\nstate = \"good\"\nvalue = \"1\"\n\n[[entity_transfer_amount]]\ntransfer = \"call\"\nstate = \"bad\"\nvalue = \"0\"\n",
        entity_static_source()
    )
}

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

#[test]
fn entity_inputs_support_both_static_and_staged_evaluation_without_source_rewriting() {
    let static_text = entity_static_source();
    let inputs = parse(&static_text, source(&static_text)).unwrap();
    assert!(inputs.entity_funding.is_some());
    assert!(inputs.funding.is_none());
    assert!(inputs.funding_schedule.is_none());
    let direct = crate::evaluate(&inputs).unwrap();
    let dispatched = evaluate(&static_text, source(&static_text)).unwrap();
    assert_eq!(dispatched.mode(), "declared_static_capital_comparison");
    assert_eq!(dispatched.to_json(), direct.to_json());
    assert_eq!(dispatched.to_markdown(), direct.to_markdown());
    assert_eq!(
        parse(ENTITY, source(ENTITY)).unwrap_err().code,
        "FMINE-REF-506"
    );
    assert_eq!(
        evaluate(ENTITY, source(&static_text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let staged = evaluate(ENTITY, source(ENTITY)).unwrap();
    assert_eq!(staged.mode(), "declared_two_stage_capital_comparison");
    let EvaluatedModel::Recourse(staged) = staged else {
        panic!("explicit entity recourse must retain its staged decision object");
    };
    assert_eq!(staged.summary().policies.len(), 6);
    assert_eq!(
        staged.summary().net_adaptive_value_per_share.unwrap().major,
        Rat::new(9, 20).unwrap()
    );
    let funding = staged.inputs().program.entity_funding.as_ref().unwrap();
    for anchor in &funding.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(ENTITY.as_bytes()).0);
        assert!(
            anchor
                .locator
                .starts_with("declared static capital program; line ")
        );
    }
    assert!(staged.claim().parents.contains(&funding.claim_id));
    assert!(
        staged
            .static_program()
            .claim()
            .parents
            .contains(&funding.claim_id)
    );
}

#[test]
fn entity_claim_binds_complete_native_transfer_distribution_sources_and_metadata() {
    let text = entity_with_transfer();
    let inputs = parse(&text, source(&text)).unwrap();
    let funding = inputs.entity_funding.as_ref().unwrap();
    assert_eq!(
        funding.metric_key,
        "capital.invented_entity_funding_and_recourse.entity_funding"
    );
    assert_eq!(funding.evidence, EvidenceClass::Assumed);
    assert_eq!(funding.state, KnowledgeState::Estimated);
    assert_eq!(funding.additivity, AdditivityClass::NonAdditive);
    assert_eq!(funding.confidentiality, Confidentiality::internal());
    assert_eq!(funding.uu_terms, inputs.policy.uu_terms);
    assert_eq!(funding.value.transfers[0].amounts.len(), 2);
    assert!(
        funding.value.transfers[0]
            .amounts
            .iter()
            .any(|(state, value)| state == "bad" && value.minor == 0)
    );
    for suffix in [
        "entity funding objective, opening date and cash basis",
        "entity parent",
        "entity jv",
        "entity period first",
        "entity floor parent/first",
        "entity opening cash parent/good",
        "entity baseline cash flow jv/second/good",
        "entity candidate cash flow parent/third/project/bad",
        "entity transfer call",
        "entity transfer amount call/good",
        "entity transfer amount call/bad",
        "entity distribution jv/parent",
        "entity distribution gate jv/second/bad",
        "model_key",
        "currency",
        "UU unmodeled_jv_and_study_terms",
    ] {
        assert!(
            funding
                .anchors
                .iter()
                .any(|anchor| anchor.locator.ends_with(&format!(": {suffix}"))),
            "missing entity source anchor: {suffix}"
        );
    }
    for anchor in &funding.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(text.as_bytes()).0);
        assert_eq!(anchor.artifact_path, "synthetic/capital.toml");
    }
    let annotated = format!("{text}\n# Additional source annotation\n");
    assert_eq!(
        parse(&annotated, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let changed = parse(&annotated, source(&annotated))
        .unwrap()
        .entity_funding
        .unwrap();
    assert_eq!(changed.value, funding.value);
    assert_ne!(changed.claim_id, funding.claim_id);
}

#[test]
fn entity_parser_rejects_duplicates_for_every_declared_table() {
    let text = entity_with_transfer();
    for table in [
        "entity_funding",
        "entity",
        "entity_period",
        "entity_floor",
        "entity_opening_cash",
        "entity_baseline_cash_flow",
        "entity_candidate_cash_flow",
        "entity_transfer",
        "entity_transfer_amount",
        "entity_distribution",
        "entity_distribution_gate",
    ] {
        let duplicate = format!("{text}\n{}", first_table_block(&text, table));
        assert_eq!(
            parse(&duplicate, source(&duplicate)).unwrap_err().code,
            "FMINE-REF-506",
            "duplicate table row: {table}"
        );
    }
}

#[test]
fn entity_parser_refuses_unknown_fields_unquoted_gates_invalid_shares_and_orphans() {
    let text = entity_with_transfer();
    for invalid in [
        text.replace(
            "objective_entity = \"parent\"",
            "objective_entity = \"parent\"\nundeclared = \"value\"",
        ),
        text.replace("allowed = \"true\"", "allowed = true"),
        text.replace("allowed = \"true\"", "allowed = \"open\""),
        text.replace("parent_share = \"3/5\"", "parent_share = \"0.6\""),
        text.replace(
            "objective_entity = \"parent\"",
            "objective_entity = \"invalid entity\"",
        ),
        text.replace("transfer = \"call\"", "transfer = \"missing\""),
        format!("{text}\n[[entity_undocumented]]\nvalue = \"0\"\n"),
    ] {
        assert_ne!(invalid, text, "mutation must change the entity source");
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
    let orphan = text.replace(&first_table_block(&text, "entity_funding"), "");
    let refusal = parse(&orphan, source(&orphan)).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-506");
    assert!(refusal.what.contains("explicit entity_funding declaration"));
}

#[test]
fn entity_parser_requires_complete_matrices_and_exclusive_funding_modes() {
    let text = entity_with_transfer();
    for table in [
        "entity_floor",
        "entity_opening_cash",
        "entity_baseline_cash_flow",
        "entity_candidate_cash_flow",
        "entity_transfer_amount",
        "entity_distribution_gate",
    ] {
        let incomplete = text.replace(&first_table_block(&text, table), "");
        assert_eq!(
            parse(&incomplete, source(&incomplete)).unwrap_err().code,
            "FMINE-REF-201",
            "missing mandatory row from {table}"
        );
    }
    let header = first_table_block(&text, "entity_funding");
    let aggregate = "\n[[funding]]\ncash_period = \"2030-01-01/2030-12-31\"\ncash_basis = \"Aggregate model\"\nminimum_cash = \"0\"\n";
    let schedule = "\n[[funding_schedule]]\ncash_basis = \"Single-entity model\"\nopening_date = \"2030-01-01\"\n";
    let legacy_aggregate = include_str!("../../../../tests/capital/liquidity.toml");
    for mixed in [
        format!("{text}{aggregate}"),
        format!("{text}{schedule}"),
        format!("{STATIC}\n{header}"),
        format!("{legacy_aggregate}\n{header}"),
    ] {
        let refusal = parse(&mixed, source(&mixed)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(refusal.what.contains("only one funding"));
    }
}
