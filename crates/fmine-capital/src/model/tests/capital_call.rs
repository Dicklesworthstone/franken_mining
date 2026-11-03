//! Subscription-round source admission, dispatch and complete evidence binding.
//! Determinism: Exact (explicit contractual sequences and original source bytes).

use super::*;

const TABLES: [&str; 3] = ["entity_capital_calls", "capital_call", "capital_call_state"];

fn call_rows() -> String {
    let mut text = String::from(
        "\n[[entity_capital_calls]]\nmethod = \"pre_money_subscription\"\ncash_flow_convention = \"native_excludes_modeled_capital_calls\"\n\n[[capital_call]]\nid = \"round\"\nentity = \"jv\"\nperiod = \"second\"\nsequence = \"1\"\ncandidate = \"project\"\nbasis = \"Invented same-price subscription at a declared pre-money equity value\"\n",
    );
    for state in ["good", "bad"] {
        text.push_str(&format!(
            "\n[[capital_call_state]]\ncall = \"round\"\nstate = \"{state}\"\npre_money_value = \"100\"\nrequired_funding = \"10\"\nparent_contribution = \"0\"\nexternal_contribution = \"10\"\n"
        ));
    }
    text
}

fn static_source() -> String {
    format!("{}{}", entity_static_source(), call_rows())
}

#[test]
fn subscription_rows_dispatch_through_static_and_staged_models_with_exact_ownership() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let funding = inputs.entity_funding.as_ref().unwrap();
    let calls = funding.value.capital_calls.as_ref().unwrap();
    assert_eq!(calls.calls.len(), 1);
    assert_eq!(calls.calls[0].states.len(), 2);
    assert_eq!(calls.calls[0].sequence, 1);
    let direct = crate::evaluate(&inputs).unwrap();
    let dispatched = evaluate(&text, source(&text)).unwrap();
    assert_eq!(direct.to_json(), dispatched.to_json());
    assert_eq!(direct.to_markdown(), dispatched.to_markdown());
    for choice in &direct.summary().choices {
        for state in &choice.entity_funding.as_ref().unwrap().states {
            let call = &state.capital_calls[0];
            let active = choice.selected.iter().any(|id| id == "project");
            assert_eq!(call.active, active);
            assert_eq!(
                call.parent_share_after,
                Rat::new(if active { 6 } else { 3 }, if active { 11 } else { 5 }).unwrap()
            );
        }
    }
    let text = format!("{ENTITY}{}", call_rows());
    assert_eq!(
        parse(&text, source(&text)).unwrap_err().code,
        "FMINE-REF-506"
    );
    let EvaluatedModel::Recourse(report) = evaluate(&text, source(&text)).unwrap() else {
        panic!("explicit information must preserve the two-stage mode");
    };
    assert_eq!(
        report.summary().net_adaptive_value_per_share.unwrap().major,
        Rat::new(9, 20).unwrap()
    );
    assert!(
        report.claim().parents.contains(
            &report
                .inputs()
                .program
                .entity_funding
                .as_ref()
                .unwrap()
                .claim_id
        )
    );
}

#[test]
fn every_call_row_is_bound_to_its_original_source_and_canonical_order_is_independent() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let claim = inputs.entity_funding.as_ref().unwrap();
    assert_eq!(claim.evidence, EvidenceClass::Assumed);
    assert_eq!(claim.state, KnowledgeState::Estimated);
    assert_eq!(claim.uu_terms, inputs.policy.uu_terms);
    for row in parse_subset(&text).unwrap().rows {
        if TABLES.contains(&row.table.as_str()) {
            let locator = format!("declared static capital program; line {}: ", row.line);
            assert!(
                claim
                    .anchors
                    .iter()
                    .any(|anchor| anchor.locator.starts_with(&locator))
            );
        }
    }
    assert!(
        claim
            .anchors
            .iter()
            .all(|anchor| anchor.artifact_sha256 == sha256(text.as_bytes()).0)
    );
    let header = first_table_block(&text, "entity_capital_calls");
    let call = first_table_block(&text, "capital_call");
    let state = first_table_block(&text, "capital_call_state");
    let reordered = format!(
        "{}{state}{call}{header}",
        text.replacen(&header, "", 1)
            .replacen(&call, "", 1)
            .replacen(&state, "", 1)
    );
    let reordered = parse(&reordered, source(&reordered))
        .unwrap()
        .entity_funding
        .unwrap();
    assert_eq!(reordered.value, claim.value);
    assert_ne!(reordered.claim_id, claim.claim_id);
    let annotated = format!("{text}\n# Original contractual source annotation\n");
    assert_eq!(
        parse(&annotated, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let annotated = parse(&annotated, source(&annotated))
        .unwrap()
        .entity_funding
        .unwrap();
    assert_eq!(annotated.value, claim.value);
    assert_ne!(annotated.claim_id, claim.claim_id);
    let changed = text.replacen(
        "external_contribution = \"10\"",
        "external_contribution = \"11\"",
        1,
    );
    let changed = parse(&changed, source(&changed))
        .unwrap()
        .entity_funding
        .unwrap();
    assert_ne!(changed.value.canonical_tag(), claim.value.canonical_tag());
}

#[test]
fn call_tables_reject_duplicate_rows_unknown_fields_and_missing_declarations() {
    let text = static_source();
    for table in TABLES {
        let block = first_table_block(&text, table);
        for invalid in [
            format!("{text}\n{block}"),
            text.replacen(&block, &format!("{block}undeclared = \"value\"\n"), 1),
        ] {
            assert_eq!(
                parse(&invalid, source(&invalid)).unwrap_err().code,
                "FMINE-REF-506",
                "{table}"
            );
        }
    }
    for table in ["entity_capital_calls", "entity_funding"] {
        let invalid = text.replacen(&first_table_block(&text, table), "", 1);
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
    let incomplete = text.replacen(&first_table_block(&text, "capital_call_state"), "", 1);
    assert_eq!(
        parse(&incomplete, source(&incomplete)).unwrap_err().code,
        "FMINE-REF-201"
    );
    let empty = format!(
        "{}{}",
        entity_static_source(),
        first_table_block(&text, "entity_capital_calls")
    );
    assert_eq!(
        parse(&empty, source(&empty)).unwrap_err().code,
        "FMINE-REF-201"
    );
}

#[test]
fn calls_require_exact_declared_terms_valid_money_and_known_matrix_references() {
    let text = static_source();
    for (from, to, code) in [
        (
            "pre_money_subscription",
            "automatic_dilution",
            "FMINE-REF-506",
        ),
        (
            "native_excludes_modeled_capital_calls",
            "native_includes_calls",
            "FMINE-REF-506",
        ),
        ("sequence = \"1\"", "sequence = 1", "FMINE-REF-506"),
        ("sequence = \"1\"", "sequence = \"0\"", "FMINE-REF-506"),
        ("sequence = \"1\"", "sequence = \"65\"", "FMINE-REF-506"),
        ("sequence = \"1\"", "sequence = \"+1\"", "FMINE-REF-506"),
        ("sequence = \"1\"", "sequence = \"1/1\"", "FMINE-REF-506"),
        ("sequence = \"1\"\n", "", "FMINE-REF-506"),
        (
            "pre_money_value = \"100\"",
            "pre_money_value = \"0\"",
            "FMINE-REF-105",
        ),
        (
            "required_funding = \"10\"",
            "required_funding = \"-1\"",
            "FMINE-REF-105",
        ),
        (
            "parent_contribution = \"0\"",
            "parent_contribution = \"-1\"",
            "FMINE-REF-105",
        ),
        (
            "external_contribution = \"10\"",
            "external_contribution = \"1/2\"",
            "FMINE-REF-506",
        ),
        ("call = \"round\"", "call = \"bad call\"", "FMINE-REF-506"),
        ("call = \"round\"", "call = \"missing\"", "FMINE-REF-506"),
    ] {
        let invalid = text.replace(from, to);
        assert_ne!(invalid, text);
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            code,
            "{from} -> {to}"
        );
    }
    let block = first_table_block(&text, "capital_call");
    for (from, to) in [
        ("entity = \"jv\"", "entity = \"parent\""),
        ("period = \"second\"", "period = \"missing\""),
        ("candidate = \"project\"", "candidate = \"missing\""),
    ] {
        let invalid = text.replacen(&block, &block.replace(from, to), 1);
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            "FMINE-REF-105"
        );
    }
    let state = first_table_block(&text, "capital_call_state");
    let invalid = text.replacen(
        &state,
        &state.replace("state = \"good\"", "state = \"missing\""),
        1,
    );
    assert_eq!(
        parse(&invalid, source(&invalid)).unwrap_err().code,
        "FMINE-REF-105"
    );
    let unconditional = text.replacen(&block, &block.replace("candidate = \"project\"\n", ""), 1);
    assert!(
        parse(&unconditional, source(&unconditional))
            .unwrap()
            .entity_funding
            .unwrap()
            .value
            .capital_calls
            .unwrap()
            .calls[0]
            .candidate_id
            .is_none()
    );
}

#[test]
fn calls_cannot_hide_sequence_collisions_or_exceed_the_input_work_domain() {
    let text = static_source();
    let second = call_rows()
        .replace("round", "second_round")
        .replace(&first_table_block(&call_rows(), "entity_capital_calls"), "");
    let collision = format!("{text}{second}");
    assert_eq!(
        parse(&collision, source(&collision)).unwrap_err().code,
        "FMINE-REF-105"
    );
    let distinct = format!(
        "{text}{}",
        second.replace("sequence = \"1\"", "sequence = \"2\"")
    );
    assert_eq!(
        parse(&distinct, source(&distinct))
            .unwrap()
            .entity_funding
            .unwrap()
            .value
            .capital_calls
            .unwrap()
            .calls
            .len(),
        2
    );
    let mut oversized = entity_static_source();
    oversized.push_str(&first_table_block(&text, "entity_capital_calls"));
    let template = first_table_block(&text, "capital_call");
    for index in 0..65 {
        oversized.push_str(&template.replace("id = \"round\"", &format!("id = \"round_{index}\"")));
    }
    assert_eq!(
        parse(&oversized, source(&oversized)).unwrap_err().code,
        "FMINE-REF-506"
    );
}
