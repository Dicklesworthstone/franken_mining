//! Strict debt-source admission and binding through both public capital model entry points.
//! Determinism: Exact (original bytes, explicit matrix rows and exact debt declarations).

use super::*;

const DEBT: &str = include_str!("../../../../../tests/capital/debt_funding.toml");
const DEBT_TABLES: [&str; 5] = [
    "entity_debt",
    "debt_facility",
    "debt_opening_principal",
    "debt_service",
    "candidate_debt_service",
];

fn static_source() -> String {
    let staged = DEBT.find("[[recourse]]").unwrap();
    let unknowns = staged + DEBT[staged..].find("[[uu]]").unwrap();
    format!("{}{}", &DEBT[..staged], &DEBT[unknowns..])
}

fn replace_in_table(text: &str, table: &str, from: &str, to: &str) -> String {
    let block = first_table_block(text, table);
    let replacement = block.replacen(from, to, 1);
    assert_ne!(block, replacement, "mutation must change {table}");
    text.replacen(&block, &replacement, 1)
}

#[test]
fn debt_model_dispatch_preserves_static_and_staged_results_and_original_source() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let funding = inputs.entity_funding.as_ref().unwrap();
    let debt = funding.value.debt.as_ref().unwrap();
    assert_eq!(debt.facilities.len(), 1);
    assert_eq!(debt.opening_principal.len(), 2);
    assert_eq!(debt.baseline_flows.len(), 6);
    assert_eq!(debt.candidate_flows.len(), 12);
    let direct = crate::evaluate(&inputs).unwrap();
    let dispatched = evaluate(&text, source(&text)).unwrap();
    assert_eq!(dispatched.mode(), "declared_static_capital_comparison");
    assert_eq!(dispatched.to_json(), direct.to_json());
    assert_eq!(dispatched.to_markdown(), direct.to_markdown());
    assert_eq!(dispatched.no_claim(), direct.no_claim());

    assert_eq!(parse(DEBT, source(DEBT)).unwrap_err().code, "FMINE-REF-506");
    assert_eq!(
        evaluate(DEBT, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let staged = evaluate(DEBT, source(DEBT)).unwrap();
    assert_eq!(staged.mode(), "declared_two_stage_capital_comparison");
    let EvaluatedModel::Recourse(staged) = staged else {
        panic!("debt must retain the explicit staged decision mode");
    };
    let funding = staged.inputs().program.entity_funding.as_ref().unwrap();
    assert!(staged.claim().parents.contains(&funding.claim_id));
    assert!(
        staged
            .static_program()
            .claim()
            .parents
            .contains(&funding.claim_id)
    );
    assert_eq!(
        staged.summary().net_adaptive_value_per_share.unwrap().major,
        Rat::new(9, 20).unwrap()
    );
    for anchor in &funding.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(DEBT.as_bytes()).0);
    }
}

#[test]
fn debt_claim_binds_every_original_row_and_inherits_evidence_and_unknowns() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let funding = inputs.entity_funding.as_ref().unwrap();
    assert_eq!(funding.evidence, EvidenceClass::Assumed);
    assert_eq!(funding.state, KnowledgeState::Estimated);
    assert_eq!(funding.additivity, AdditivityClass::NonAdditive);
    assert_eq!(funding.confidentiality, Confidentiality::internal());
    assert_eq!(funding.uu_terms, inputs.policy.uu_terms);
    assert!(!funding.uu_terms.is_empty());
    for row in parse_subset(&text).unwrap().rows {
        if DEBT_TABLES.contains(&row.table.as_str()) {
            let prefix = format!("declared static capital program; line {}: ", row.line);
            assert!(
                funding
                    .anchors
                    .iter()
                    .any(|anchor| anchor.locator.starts_with(&prefix)),
                "missing original debt source row {}: {}",
                row.line,
                row.table
            );
        }
    }
    for suffix in [
        "entity debt cash-flow convention",
        "debt facility loan",
        "debt opening principal loan/bad",
        "debt service loan/second/bad",
        "candidate debt service loan/third/study/bad",
        "candidate debt service loan/third/project/bad",
    ] {
        assert!(
            funding
                .anchors
                .iter()
                .any(|anchor| anchor.locator.ends_with(&format!(": {suffix}"))),
            "missing debt source locator: {suffix}"
        );
    }
    for anchor in &funding.anchors {
        assert_eq!(anchor.artifact_sha256, sha256(text.as_bytes()).0);
        assert_eq!(anchor.artifact_path, "synthetic/capital.toml");
    }
    let annotated = format!("{text}\n# Original debt source annotation\n");
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
    let changed_value = replace_in_table(
        &text,
        "candidate_debt_service",
        "cfads = \"0\"",
        "cfads = \"1\"",
    );
    let changed = parse(&changed_value, source(&changed_value))
        .unwrap()
        .entity_funding
        .unwrap();
    assert_ne!(changed.value.canonical_tag(), funding.value.canonical_tag());
    assert_ne!(changed.claim_id, funding.claim_id);
}

#[test]
fn debt_declarations_are_order_independent_and_keep_original_ordered_source_hashes() {
    let document = parse_subset(DEBT).unwrap();
    let lines: Vec<_> = DEBT.lines().collect();
    let mut reversed = format!("{}\n", lines[..document.rows[0].line - 1].join("\n"));
    for (index, row) in document.rows.iter().enumerate().rev() {
        let end = document
            .rows
            .get(index + 1)
            .map_or(lines.len(), |next| next.line - 1);
        reversed.push_str(&lines[row.line - 1..end].join("\n"));
        reversed.push('\n');
    }
    assert_ne!(reversed, DEBT);
    let EvaluatedModel::Recourse(original) = evaluate(DEBT, source(DEBT)).unwrap() else {
        panic!("fixture must declare recourse");
    };
    let EvaluatedModel::Recourse(reordered) = evaluate(&reversed, source(&reversed)).unwrap()
    else {
        panic!("table order must preserve recourse");
    };
    assert_eq!(original.summary(), reordered.summary());
    assert_eq!(
        original.static_program().summary(),
        reordered.static_program().summary()
    );
    assert_eq!(
        original
            .inputs()
            .program
            .entity_funding
            .as_ref()
            .unwrap()
            .value,
        reordered
            .inputs()
            .program
            .entity_funding
            .as_ref()
            .unwrap()
            .value
    );
    assert_ne!(original.digest(), reordered.digest());
    for anchor in &reordered
        .inputs()
        .program
        .entity_funding
        .as_ref()
        .unwrap()
        .anchors
    {
        assert_eq!(anchor.artifact_sha256, sha256(reversed.as_bytes()).0);
    }
}

#[test]
fn debt_parser_refuses_duplicate_rows_and_unknown_fields_in_every_table() {
    let text = static_source();
    for table in DEBT_TABLES {
        let block = first_table_block(&text, table);
        let duplicate = format!("{text}\n{block}");
        assert_eq!(
            parse(&duplicate, source(&duplicate)).unwrap_err().code,
            "FMINE-REF-506",
            "duplicate {table}"
        );
        let unknown = text.replacen(&block, &format!("{block}unregistered = \"value\"\n"), 1);
        assert_eq!(
            parse(&unknown, source(&unknown)).unwrap_err().code,
            "FMINE-REF-506",
            "unknown {table} field"
        );
    }
}

#[test]
fn debt_parser_requires_explicit_headers_boolean_and_complete_matrices() {
    let text = static_source();
    for table in ["entity_debt", "entity_funding"] {
        let orphan = text.replacen(&first_table_block(&text, table), "", 1);
        let refusal = parse(&orphan, source(&orphan)).unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-506");
        assert!(
            refusal
                .what
                .contains(&format!("explicit {table} declaration"))
        );
    }
    for table in [
        "debt_opening_principal",
        "debt_service",
        "candidate_debt_service",
    ] {
        let incomplete = text.replacen(&first_table_block(&text, table), "", 1);
        assert_eq!(
            parse(&incomplete, source(&incomplete)).unwrap_err().code,
            "FMINE-REF-201",
            "missing {table} row"
        );
    }
    let missing_flag = replace_in_table(
        &text,
        "debt_facility",
        "allow_distributions_without_service = \"true\"\n",
        "",
    );
    assert_eq!(
        parse(&missing_flag, source(&missing_flag))
            .unwrap_err()
            .code,
        "FMINE-REF-506"
    );
    let false_flag = replace_in_table(
        &text,
        "debt_facility",
        "allow_distributions_without_service = \"true\"",
        "allow_distributions_without_service = \"false\"",
    );
    let parsed = parse(&false_flag, source(&false_flag)).unwrap();
    assert!(
        !parsed
            .entity_funding
            .unwrap()
            .value
            .debt
            .unwrap()
            .facilities[0]
            .allow_distributions_without_service
    );

    let header_only = format!(
        "{}\n{}",
        entity_static_source(),
        first_table_block(&text, "entity_debt")
    );
    assert_eq!(
        parse(&header_only, source(&header_only)).unwrap_err().code,
        "FMINE-REF-201"
    );
    let foreign_funding = format!("{STATIC}\n{}", first_table_block(&text, "entity_debt"));
    assert_eq!(
        parse(&foreign_funding, source(&foreign_funding))
            .unwrap_err()
            .code,
        "FMINE-REF-506"
    );
}

#[test]
fn debt_parser_refuses_unquoted_values_invalid_ratios_and_unknown_references() {
    let text = static_source();
    for (table, from, to, code) in [
        (
            "entity_debt",
            "native_excludes_modeled_debt",
            "native_includes_debt",
            "FMINE-REF-506",
        ),
        (
            "debt_facility",
            "allow_distributions_without_service = \"true\"",
            "allow_distributions_without_service = true",
            "FMINE-REF-506",
        ),
        (
            "debt_facility",
            "allow_distributions_without_service = \"true\"",
            "allow_distributions_without_service = \"automatic\"",
            "FMINE-REF-506",
        ),
        (
            "debt_facility",
            "minimum_dscr = \"3/2\"",
            "minimum_dscr = \"1.5\"",
            "FMINE-REF-506",
        ),
        (
            "debt_facility",
            "minimum_dscr = \"3/2\"",
            "minimum_dscr = \"0/1\"",
            "FMINE-REF-105",
        ),
        (
            "debt_facility",
            "sweep_fraction = \"1/2\"",
            "sweep_fraction = \"3/2\"",
            "FMINE-REF-105",
        ),
        (
            "debt_facility",
            "entity = \"jv\"",
            "entity = \"missing\"",
            "FMINE-REF-105",
        ),
        (
            "debt_opening_principal",
            "facility = \"loan\"",
            "facility = \"bad facility\"",
            "FMINE-REF-506",
        ),
        (
            "debt_opening_principal",
            "facility = \"loan\"",
            "facility = \"missing\"",
            "FMINE-REF-506",
        ),
        (
            "debt_opening_principal",
            "state = \"good\"",
            "state = \"unknown\"",
            "FMINE-REF-105",
        ),
        (
            "debt_opening_principal",
            "value = \"100\"",
            "value = \"-1\"",
            "FMINE-REF-105",
        ),
        (
            "debt_service",
            "period = \"first\"",
            "period = \"unknown\"",
            "FMINE-REF-105",
        ),
        (
            "debt_service",
            "interest_rate = \"0/1\"",
            "interest_rate = \"1/0\"",
            "FMINE-REF-506",
        ),
        (
            "debt_service",
            "interest_rate = \"0/1\"",
            "interest_rate = \"-1/10\"",
            "FMINE-REF-105",
        ),
        (
            "debt_service",
            "draw = \"0\"",
            "draw = \"-1\"",
            "FMINE-REF-105",
        ),
        (
            "candidate_debt_service",
            "candidate = \"study\"",
            "candidate = \"unknown\"",
            "FMINE-REF-105",
        ),
        (
            "candidate_debt_service",
            "principal_payment_cap = \"0\"",
            "principal_payment_cap = \"-1\"",
            "FMINE-REF-105",
        ),
    ] {
        let invalid = replace_in_table(&text, table, from, to);
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            code,
            "{table}: {to}"
        );
    }
}

#[test]
fn debt_facility_count_is_bounded_during_source_admission() {
    let mut text = static_source();
    let facility = first_table_block(&text, "debt_facility");
    for index in 1..=4 {
        text.push_str(&facility.replace("id = \"loan\"", &format!("id = \"loan{index}\"")));
    }
    let refusal = parse(&text, source(&text)).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-506");
    assert!(refusal.what.contains("4-facility"));
}
