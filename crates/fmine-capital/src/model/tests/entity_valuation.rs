//! Strict source-bound additional entity valuations through static and staged entry points.
//! Determinism: Exact (independent perimeters, original source bytes and complete matrices).

use super::*;

const TABLES: [&str; 4] = [
    "entity_valuation",
    "entity_baseline_nav",
    "entity_candidate_payoff",
    "entity_candidate_share_change",
];

fn valuation_rows(entity: &str, cap: Option<&str>) -> String {
    let mut rows = format!(
        "\n[[entity_valuation]]\nentity = \"{entity}\"\nshare_class = \"ordinary\"\nvaluation_basis = \"Independent invented carve-out NAV including already-net financing and project effects\"\nshares_per_unit = \"1/1\"\ninitial_share_units = \"60/1\"\n"
    );
    if let Some(cap) = cap {
        rows.push_str(&format!("max_tail_loss_per_share = \"{cap}\"\n"));
    }
    for state in ["good", "bad"] {
        rows.push_str(&format!(
            "\n[[entity_baseline_nav]]\nentity = \"{entity}\"\nstate = \"{state}\"\nvalue = \"600\"\n"
        ));
        for candidate in ["study", "project"] {
            let nav = match (candidate, state) {
                ("study", _) => "-6",
                ("project", "good") => "126",
                _ => "-54",
            };
            let shares = if candidate == "study" { "0/1" } else { "6/1" };
            rows.push_str(&format!(
                "\n[[entity_candidate_payoff]]\nentity = \"{entity}\"\ncandidate = \"{candidate}\"\nstate = \"{state}\"\nvalue = \"{nav}\"\n\n[[entity_candidate_share_change]]\nentity = \"{entity}\"\ncandidate = \"{candidate}\"\nstate = \"{state}\"\nvalue = \"{shares}\"\n"
            ));
        }
    }
    rows
}

fn static_source() -> String {
    format!(
        "{}{}",
        entity_static_source(),
        valuation_rows("carve_out", Some("1/10"))
    )
}

fn staged_source() -> String {
    format!("{ENTITY}{}", valuation_rows("carve_out", Some("1/10")))
}

fn replace_in_table(text: &str, table: &str, from: &str, to: &str) -> String {
    let block = first_table_block(text, table);
    let changed = block.replacen(from, to, 1);
    assert_ne!(changed, block, "mutation must change {table}");
    text.replacen(&block, &changed, 1)
}

#[test]
fn independent_entity_models_dispatch_with_exact_separate_static_and_policy_results() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    assert_eq!(inputs.entity_valuations.len(), 1);
    let declaration = &inputs.entity_valuations[0].value;
    assert_eq!(declaration.equity.entity, "carve_out");
    assert_eq!(declaration.equity.baseline_nav.len(), 2);
    assert_eq!(declaration.equity.share_changes.len(), 4);
    assert_eq!(declaration.nav_changes.len(), 4);
    assert_ne!(
        declaration.equity.valuation_basis,
        inputs.policy.value.valuation_basis
    );
    let direct = crate::evaluate(&inputs).unwrap();
    let dispatched = evaluate(&text, source(&text)).unwrap();
    assert_eq!(direct.to_json(), dispatched.to_json());
    assert_eq!(direct.to_markdown(), dispatched.to_markdown());
    assert!(direct.to_json().contains("\"entity_valuations\":["));
    assert!(
        direct
            .to_markdown()
            .contains("Additional entity valuations and binding risk limits")
    );

    let text = staged_source();
    assert_eq!(
        parse(&text, source(&text)).unwrap_err().code,
        "FMINE-REF-506"
    );
    let EvaluatedModel::Recourse(report) = evaluate(&text, source(&text)).unwrap() else {
        panic!("declared information must retain two-stage evaluation");
    };
    let winner = report
        .summary()
        .policies
        .iter()
        .find(|policy| {
            report
                .summary()
                .feasible_winners
                .contains(&policy.policy_id)
        })
        .unwrap();
    assert_eq!(
        winner.per_share_statistics.as_ref().unwrap().mean.major,
        Rat::new(9, 20).unwrap()
    );
    assert_eq!(
        winner.entity_valuations[0]
            .per_share_statistics
            .as_ref()
            .unwrap()
            .mean
            .major,
        Rat::new(89, 220).unwrap()
    );
    assert_eq!(
        winner.entity_valuations[0]
            .per_share_tail_loss
            .unwrap()
            .major,
        Rat::new(1, 10).unwrap()
    );
    assert!(!winner.entity_valuations[0].risk_rejected);
    assert_eq!(report.summary().policies.len(), 6);
    assert_eq!(report.static_program().summary().choices.len(), 4);
}

#[test]
fn each_entity_claim_retains_its_own_original_rows_and_shared_evidence() {
    let text = format!(
        "{}{}{}",
        entity_static_source(),
        valuation_rows("alpha", None),
        valuation_rows("zeta", Some("1/10"))
    );
    let inputs = parse(&text, source(&text)).unwrap();
    assert_eq!(inputs.entity_valuations.len(), 2);
    for claim in &inputs.entity_valuations {
        let entity = &claim.value.equity.entity;
        assert_eq!(
            claim.metric_key,
            format!(
                "capital.{}.entity_valuation.{entity}",
                inputs.policy.value.model_key
            )
        );
        assert_eq!(claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.state, KnowledgeState::Estimated);
        assert_eq!(claim.additivity, AdditivityClass::NonAdditive);
        assert_eq!(claim.confidentiality, Confidentiality::internal());
        assert_eq!(claim.uu_terms, inputs.policy.uu_terms);
        for row in parse_subset(&text).unwrap().rows {
            if !TABLES.contains(&row.table.as_str()) {
                continue;
            }
            let row_entity = row
                .fields
                .iter()
                .find(|field| field.key == "entity")
                .unwrap();
            let TomlValue::Str(row_entity) = &row_entity.value else {
                panic!("entity IDs must be quoted");
            };
            let prefix = format!("declared static capital program; line {}: ", row.line);
            assert_eq!(
                claim
                    .anchors
                    .iter()
                    .any(|anchor| anchor.locator.starts_with(&prefix)),
                row_entity == entity
            );
        }
        for anchor in &claim.anchors {
            assert_eq!(anchor.artifact_sha256, sha256(text.as_bytes()).0);
            assert_eq!(anchor.artifact_path, "synthetic/capital.toml");
        }
        for suffix in ["model_key", "currency", "payoff_convention"] {
            assert!(
                claim
                    .anchors
                    .iter()
                    .any(|anchor| anchor.locator.ends_with(&format!(": {suffix}")))
            );
        }
    }
    let report = crate::evaluate(&inputs).unwrap();
    for claim in &inputs.entity_valuations {
        assert!(report.claim().parents.contains(&claim.claim_id));
    }
    let staged = staged_source();
    let EvaluatedModel::Recourse(report) = evaluate(&staged, source(&staged)).unwrap() else {
        panic!("fixture is staged");
    };
    for claim in &report.inputs().program.entity_valuations {
        assert!(report.claim().parents.contains(&claim.claim_id));
        assert!(
            report
                .static_program()
                .claim()
                .parents
                .contains(&claim.claim_id)
        );
    }
}

#[test]
fn independent_entity_source_and_values_bind_receipts_without_changing_primary_values() {
    let text = static_source();
    let inputs = parse(&text, source(&text)).unwrap();
    let original = crate::evaluate(&inputs).unwrap();
    let changed = format!("{text}\n# independent valuation annotation\n");
    assert_eq!(
        parse(&changed, source(&text)).unwrap_err().code,
        "FMINE-REF-505"
    );
    let annotated = crate::evaluate(&parse(&changed, source(&changed)).unwrap()).unwrap();
    assert_eq!(original.summary(), annotated.summary());
    assert_ne!(original.digest(), annotated.digest());
    assert_ne!(
        inputs.entity_valuations[0].claim_id,
        annotated.inputs().entity_valuations[0].claim_id
    );

    let changed = replace_in_table(
        &text,
        "entity_candidate_payoff",
        "value = \"-6\"",
        "value = \"-7\"",
    );
    let changed = parse(&changed, source(&changed)).unwrap();
    assert_eq!(
        inputs.equity.as_ref().unwrap().value,
        changed.equity.as_ref().unwrap().value
    );
    assert_eq!(
        inputs
            .candidates
            .iter()
            .map(|claim| &claim.value)
            .collect::<Vec<_>>(),
        changed
            .candidates
            .iter()
            .map(|claim| &claim.value)
            .collect::<Vec<_>>()
    );
    assert_ne!(
        inputs.entity_valuations[0].value.canonical_tag(),
        changed.entity_valuations[0].value.canonical_tag()
    );
    assert_ne!(
        original.digest(),
        crate::evaluate(&changed).unwrap().digest()
    );
}

#[test]
fn reordered_entity_declarations_preserve_values_and_outcomes_but_bind_new_source_bytes() {
    let text = format!(
        "{ENTITY}{}{}",
        valuation_rows("zeta", None),
        valuation_rows("alpha", Some("1/10"))
    );
    let document = parse_subset(&text).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let mut reversed = format!("{}\n", lines[..document.rows[0].line - 1].join("\n"));
    for (index, row) in document.rows.iter().enumerate().rev() {
        let end = document
            .rows
            .get(index + 1)
            .map_or(lines.len(), |next| next.line - 1);
        reversed.push_str(&lines[row.line - 1..end].join("\n"));
        reversed.push('\n');
    }
    let EvaluatedModel::Recourse(original) = evaluate(&text, source(&text)).unwrap() else {
        panic!("staged fixture");
    };
    let EvaluatedModel::Recourse(reordered) = evaluate(&reversed, source(&reversed)).unwrap()
    else {
        panic!("staged fixture");
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
            .entity_valuations
            .iter()
            .map(|claim| &claim.value)
            .collect::<Vec<_>>(),
        reordered
            .inputs()
            .program
            .entity_valuations
            .iter()
            .map(|claim| &claim.value)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        original.inputs().program.entity_valuations[0]
            .value
            .equity
            .entity,
        "alpha"
    );
    assert_ne!(original.digest(), reordered.digest());
}

#[test]
fn strict_entity_rows_refuse_duplicates_unknown_fields_references_and_unquoted_values() {
    let text = static_source();
    for table in TABLES {
        let duplicate = format!("{text}\n{}", first_table_block(&text, table));
        assert_eq!(
            parse(&duplicate, source(&duplicate)).unwrap_err().code,
            "FMINE-REF-506"
        );
    }
    let cases = [
        replace_in_table(
            &text,
            "entity_valuation",
            "share_class = \"ordinary\"",
            "share_class = \"ordinary\"\nunknown = \"field\"",
        ),
        replace_in_table(
            &text,
            "entity_valuation",
            "entity = \"carve_out\"",
            "entity = \"invalid entity\"",
        ),
        replace_in_table(
            &text,
            "entity_valuation",
            "shares_per_unit = \"1/1\"",
            "shares_per_unit = \"1.0\"",
        ),
        replace_in_table(
            &text,
            "entity_valuation",
            "initial_share_units = \"60/1\"",
            "initial_share_units = \"60\"",
        ),
        replace_in_table(
            &text,
            "entity_valuation",
            "max_tail_loss_per_share = \"1/10\"",
            "max_tail_loss_per_share = \"1/0\"",
        ),
        replace_in_table(
            &text,
            "entity_baseline_nav",
            "state = \"good\"",
            "state = \"foreign\"",
        ),
        replace_in_table(
            &text,
            "entity_candidate_payoff",
            "candidate = \"study\"",
            "candidate = \"foreign\"",
        ),
        replace_in_table(
            &text,
            "entity_candidate_share_change",
            "state = \"good\"",
            "state = \"foreign\"",
        ),
        replace_in_table(
            &text,
            "entity_candidate_share_change",
            "value = \"0/1\"",
            "value = 0",
        ),
        replace_in_table(
            &text,
            "entity_candidate_payoff",
            "value = \"-6\"",
            "value = \"-6.00001\"",
        ),
    ];
    for changed in cases {
        assert_eq!(
            parse(&changed, source(&changed)).unwrap_err().code,
            "FMINE-REF-506",
            "{changed}"
        );
    }
}

#[test]
fn entity_rows_require_complete_distinct_declared_equity_perimeters() {
    let text = static_source();
    for table in &TABLES[1..] {
        let missing = text.replacen(&first_table_block(&text, table), "", 1);
        assert_eq!(
            parse(&missing, source(&missing)).unwrap_err().code,
            "FMINE-REF-201"
        );
    }
    let orphan = text.replacen(&first_table_block(&text, "entity_valuation"), "", 1);
    assert_eq!(
        parse(&orphan, source(&orphan)).unwrap_err().code,
        "FMINE-REF-506"
    );
    let no_primary = format!("{STATIC}{}", valuation_rows("carve_out", None));
    assert_eq!(
        parse(&no_primary, source(&no_primary)).unwrap_err().code,
        "FMINE-REF-105"
    );
    let duplicate_primary = text.replace("entity = \"carve_out\"", "entity = \"parent\"");
    assert_eq!(
        parse(&duplicate_primary, source(&duplicate_primary))
            .unwrap_err()
            .code,
        "FMINE-REF-105"
    );
    for (field, value) in [("shares_per_unit", "0/1"), ("initial_share_units", "-1/1")] {
        let original = if field == "shares_per_unit" {
            "1/1"
        } else {
            "60/1"
        };
        let invalid = replace_in_table(
            &text,
            "entity_valuation",
            &format!("{field} = \"{original}\""),
            &format!("{field} = \"{value}\""),
        );
        assert_eq!(
            parse(&invalid, source(&invalid)).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
    let too_many = format!(
        "{text}{}{}{}",
        valuation_rows("second", None),
        valuation_rows("third", None),
        valuation_rows("fourth", None)
    );
    assert_eq!(
        parse(&too_many, source(&too_many)).unwrap_err().code,
        "FMINE-REF-506"
    );
}

#[test]
fn secondary_risk_cap_changes_the_selected_policy_without_changing_primary_economics() {
    let text = staged_source();
    let constrained = replace_in_table(
        &text,
        "entity_valuation",
        "max_tail_loss_per_share = \"1/10\"",
        "max_tail_loss_per_share = \"0/1\"",
    );
    let EvaluatedModel::Recourse(original) = evaluate(&text, source(&text)).unwrap() else {
        panic!("staged fixture");
    };
    let EvaluatedModel::Recourse(constrained) =
        evaluate(&constrained, source(&constrained)).unwrap()
    else {
        panic!("staged fixture");
    };
    assert_eq!(
        original.summary().unconstrained_winners,
        constrained.summary().unconstrained_winners
    );
    assert_ne!(
        original.summary().feasible_winners,
        constrained.summary().feasible_winners
    );
    for (original, constrained) in original
        .summary()
        .policies
        .iter()
        .zip(&constrained.summary().policies)
    {
        assert_eq!(original.equity, constrained.equity);
        assert_eq!(
            original.per_share_statistics,
            constrained.per_share_statistics
        );
        assert_eq!(original.entity_funding, constrained.entity_funding);
        if !original.initial_selected.is_empty() && original.structural_rejections.is_empty() {
            assert!(constrained.entity_valuations[0].risk_rejected);
            assert!(constrained.risk_rejected);
        }
    }
    for winner in &constrained.summary().feasible_winners {
        let choice = constrained
            .summary()
            .policies
            .iter()
            .find(|policy| &policy.policy_id == winner)
            .unwrap();
        assert!(choice.states.iter().all(|state| state.selected.is_empty()));
    }
}
