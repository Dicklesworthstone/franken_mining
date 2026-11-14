//! Constructed PDF cost-rate identities through the registry, source boundary and exact joint solver.
//! These known answers do not authenticate a real filing or establish an accounting definition.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use fmine_metrics::roll_forward::{self, TableReport, cost_bridge};
use fmine_numerics::exact_feas::{Outcome, Rat};
use fmine_registry::sources::DocumentRow;
use fmine_registry::table_identities::{TableIdentityRegistry, parse_table_identities};
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};

const BASIS: &str = "attributable;denominator=sold";
const PERIOD: &str = "2025FY";

fn declaration(copper: bool) -> String {
    let (kind, unit) = if copper {
        ("copper_cost_per_sold_lb", "usd_per_lb")
    } else {
        ("gold_cost_per_sold_oz", "usd_per_oz")
    };
    format!(
        r#"schema = "fmine.table-identities.v1"
issuer = "synthetic"
[[identity]]
id = "cost_rate"
doc = "Invented.pdf"
family = "reported_cost_rate"
kind = "{kind}"
total = "rate"
total_period = "{PERIOD}"
unit = "{unit}"
basis = "{BASIS}"
construction = "rounded_once"
convention = "closed"
[[term]]
identity = "cost_rate"
quantity = "cost"
period = "{PERIOD}"
sign = 1
role = "numerator"
[[term]]
identity = "cost_rate"
quantity = "sold"
period = "{PERIOD}"
sign = 1
role = "denominator"
"#
    )
}

const SUM: &str = r#"
[[identity]]
id = "cost_sum"
doc = "Invented.pdf"
family = "reported_cost_components"
kind = "sum"
total = "cost"
total_period = "2025FY"
unit = "usd_m"
basis = "attributable;denominator=sold"
construction = "rounded_once"
convention = "closed"
[[term]]
identity = "cost_sum"
quantity = "cost_a"
period = "2025FY"
sign = 1
role = "member"
[[term]]
identity = "cost_sum"
quantity = "cost_b"
period = "2025FY"
sign = 1
role = "member"
"#;

fn document() -> DocumentRow {
    DocumentRow {
        file: "Invented.pdf".into(),
        dir: "synthetic".into(),
        kind: "partner_filing".into(),
        url: "https://example.invalid/Invented.pdf".into(),
        url_basis: "DESIGN: invented pure arithmetic fixture".into(),
        published_at: "2026-02-01".into(),
        published_at_basis: "FACT: constructed fixture date".into(),
        pages: 1,
        bytes: 1,
        sha256: "11".repeat(32),
    }
}

fn reading(quantity: &str, token: &str, unit: &str) -> Accepted {
    Accepted {
        locator: Locator {
            doc: document().file,
            page: 1,
            table: "Invented cost reconciliation".into(),
            row: quantity.into(),
            col: PERIOD.into(),
        },
        doc_sha256: document().sha256,
        token: token.into(),
        value: parse_token(token).expect("exact printed token"),
        quantity: quantity.into(),
        unit: unit.into(),
        basis: BASIS.into(),
        period: PERIOD.into(),
        readers: vec!["reader_a".into(), "reader_b".into()],
    }
}

fn readings(copper: bool) -> Vec<Accepted> {
    vec![
        reading("cost", "100.000", "usd_m"),
        reading("sold", "50.000", if copper { "Mlb" } else { "koz" }),
        reading(
            "rate",
            if copper { "2.000" } else { "2000.0" },
            if copper { "usd_per_lb" } else { "usd_per_oz" },
        ),
    ]
}

fn registry(text: &str) -> TableIdentityRegistry {
    parse_table_identities(text, "synthetic", &[document()]).expect("declared cost-rate identity")
}

fn assess(registry: &TableIdentityRegistry, values: &[Accepted]) -> TableReport {
    roll_forward::assess(registry, values, &[document()])
}

#[test]
fn gold_and_copper_native_scales_keep_exact_checked_witnesses_and_signed_costs() {
    for copper in [false, true] {
        let declared = registry(&declaration(copper));
        for negative in [false, true] {
            let mut values = readings(copper);
            if negative {
                for index in [0, 2] {
                    values[index].token = format!("({})", values[index].token);
                    values[index].value = parse_token(&values[index].token).unwrap();
                }
            }
            let report = assess(&declared, &values);
            assert!(report.refused.is_empty(), "{}", report.to_json());
            assert_eq!(report.components.len(), 1);
            let Outcome::Feasible { witness } = &report.components[0].component.outcome else {
                panic!("{}", report.to_json());
            };
            let amount = |index: usize| witness[&values[index].locator.to_string()];
            let expected = amount(0)
                .checked_mul(Rat::from_integer(if copper { 1 } else { 1000 }))
                .unwrap()
                .checked_div(amount(1))
                .unwrap();
            assert_eq!(amount(2), expected);
            assert_eq!(amount(2).signum(), if negative { -1 } else { 1 });
            assert!(
                report
                    .to_json()
                    .contains("\"verdict\":\"feasible\",\"checked\":true")
            );
            assert!(report.to_json().contains(if copper {
                "\"denominator_unit\":\"Mlb\",\"unit\":\"usd_per_lb\",\"factor\":1"
            } else {
                "\"denominator_unit\":\"koz\",\"unit\":\"usd_per_oz\",\"factor\":1000"
            }));
            assert!(report.metric_definitions.is_empty());
            assert_eq!(report.cells.len(), 3);
            assert!(report.cells.iter().any(|c| c.source == values[0]));
            assert_eq!(
                report.digest(),
                assess(&declared, &values.into_iter().rev().collect::<Vec<_>>()).digest()
            );
        }
    }
}

#[test]
fn amount_sum_and_cost_rate_are_checked_jointly_and_keep_a_conflict_certificate() {
    let declared = registry(&format!("{}{SUM}", declaration(false)));
    let mut values = readings(false);
    values[0] = reading("cost", "100", "usd_m");
    values.extend([
        reading("cost_a", "50.20", "usd_m"),
        reading("cost_b", "50.20", "usd_m"),
    ]);
    // The subtotal permits roughly 100.4; the rate permits roughly 100.0. Each fits the
    // separately printed 100, but there is no common unrounded cost satisfying both.
    for identity in &declared.identities {
        let mut single = declared.clone();
        single.identities = vec![identity.clone()];
        let report = assess(&single, &values);
        assert!(report.refused.is_empty(), "{}", report.to_json());
        assert!(matches!(
            report.components[0].component.outcome,
            Outcome::Feasible { .. }
        ));
    }
    let joint = assess(&declared, &values);
    assert!(joint.refused.is_empty());
    assert_eq!(joint.components.len(), 1);
    assert!(matches!(
        joint.components[0].component.outcome,
        Outcome::Infeasible { .. }
    ));
    assert!(joint.components[0].certificate_digest.is_some());
    assert!(
        joint
            .to_json()
            .contains("\"verdict\":\"infeasible\",\"checked\":true")
    );
    values[2] = reading("rate", "2008.0", "usd_per_oz");
    let compatible = assess(&declared, &values);
    assert!(matches!(
        compatible.components[0].component.outcome,
        Outcome::Feasible { .. }
    ));
    assert!(
        compatible.components[0]
            .implied
            .iter()
            .all(|(_, interval)| interval.is_ok())
    );
}

#[test]
fn a_boundary_only_cost_rate_respects_the_declared_tie_convention() {
    let values = vec![
        reading("cost", "2.3", "usd_m"),
        reading("sold", "1", "Mlb"),
        reading("rate", "1", "usd_per_lb"),
    ];
    // Closed boxes meet only at cost=2.25, sold=1.5 and rate=1.5. A half-up
    // convention excludes the upper sold/rate endpoints, so it cannot retain that witness.
    let closed = assess(&registry(&declaration(true)), &values);
    let Outcome::Feasible { witness } = &closed.components[0].component.outcome else {
        panic!("{}", closed.to_json());
    };
    assert_eq!(
        witness[&values[0].locator.to_string()],
        Rat::new(9, 4).unwrap()
    );
    assert_eq!(
        witness[&values[1].locator.to_string()],
        Rat::new(3, 2).unwrap()
    );
    let half_up = assess(
        &registry(
            &declaration(true).replace("convention = \"closed\"", "convention = \"half_up\""),
        ),
        &values,
    );
    assert!(half_up.refused.is_empty());
    assert!(matches!(
        half_up.components[0].component.outcome,
        Outcome::Infeasible { .. }
    ));
    assert!(half_up.components[0].certificate_digest.is_some());
}

#[test]
fn changing_either_amount_moves_the_arithmetic_finding_and_source_digest() {
    for copper in [false, true] {
        let declared = registry(&declaration(copper));
        let values = readings(copper);
        let original = assess(&declared, &values);
        for (index, token) in [(0, "110.000"), (1, "40.000")] {
            let mut changed = values.clone();
            changed[index].token = token.into();
            changed[index].value = parse_token(token).unwrap();
            let report = assess(&declared, &changed);
            assert!(report.refused.is_empty());
            assert!(matches!(
                report.components[0].component.outcome,
                Outcome::Infeasible { .. }
            ));
            assert_ne!(original.digest(), report.digest());
            assert!(report.cells.iter().any(|c| c.source == changed[index]));
        }
    }
}

#[test]
fn source_units_basis_period_hash_and_reader_guards_cannot_be_bypassed_by_a_rate_kind() {
    for copper in [false, true] {
        let declared = registry(&declaration(copper));
        for case in 0..8 {
            let mut values = readings(copper);
            match case {
                0 => values[0].unit = "usd".into(),
                1 => values[1].unit = if copper { "kt" } else { "oz" }.into(),
                2 => values[1].basis = "100%;denominator=sold".into(),
                3 => values[1].period = "2024FY".into(),
                4 => values[0].doc_sha256 = "22".repeat(32),
                5 => values[0].readers = vec!["reader_a".into()],
                6 => values[0].readers = vec!["reader_a".into(), "reader_a".into()],
                _ => values[0].locator.page = 2,
            }
            let report = assess(&declared, &values);
            assert_eq!(report.refused.len(), 1, "case {case}: {}", report.to_json());
            assert_eq!(report.refused[0].identity, "cost_rate");
            assert!(!report.refused[0].evidence.is_empty());
            assert!(report.components.is_empty());
        }
    }
}

#[test]
fn missing_ambiguous_or_nonpositive_sold_amounts_never_become_numeric_results() {
    let declared = registry(&declaration(false));
    let original = readings(false);
    let mut missing = original.clone();
    missing.remove(1);
    let mut duplicate = original.clone();
    let mut alternative = original[1].clone();
    alternative.locator.row = "second sold amount".into();
    duplicate.push(alternative);
    for values in [missing, duplicate] {
        let report = assess(&declared, &values);
        assert_eq!(report.refused.len(), 1);
        assert!(report.components.is_empty());
    }
    for token in ["0", "0.0", "(50.000)"] {
        let mut values = original.clone();
        values[1] = reading("sold", token, "koz");
        let report = assess(&declared, &values);
        assert_eq!(report.refused.len(), 1);
        assert!(
            report.refused[0]
                .reason
                .contains("strictly positive and bounded away from zero")
        );
        assert!(report.components.is_empty());
    }
}

#[test]
fn declarations_cannot_choose_a_rescuing_scale_period_sign_or_shared_rate() {
    let good = declaration(false);
    for changed in [
        good.replace("unit = \"usd_per_oz\"", "unit = \"usd_per_lb\""),
        good.replace("period = \"2025FY\"\nsign", "period = \"2024FY\"\nsign"),
        good.replace("role = \"denominator\"", "role = \"numerator\""),
        good.replace("sign = 1", "sign = -1"),
        good.replace("rounded_once", "component_sum"),
        good.replace(
            "convention = \"closed\"",
            "convention = \"closed\"\nfactor = 1",
        ),
    ] {
        assert!(parse_table_identities(&changed, "synthetic", &[document()]).is_err());
    }
    let shared = format!(
        "{good}{}",
        SUM.replace("quantity = \"cost_a\"", "quantity = \"rate\"")
    );
    let refusal = parse_table_identities(&shared, "synthetic", &[document()]).unwrap_err();
    assert!(refusal.reason.contains("quoted cost rate cannot be reused"));
    // Public callers constructing structs must satisfy the same admission rules as TOML inputs.
    let mut constructed = registry(&good);
    constructed.identities[0].terms[0].sign = -1;
    assert!(constructed.validate(&[document()]).is_err());
    assert!(assess(&constructed, &readings(false)).components.is_empty());
}

const BRIDGE_BASIS: &str = "subject=synthetic_mine;ownership=attributable:1/2;definition=synthetic_cost@v1;denominator=sold";

fn bridge_sources(copper: bool) -> (TableIdentityRegistry, Vec<Accepted>, Vec<DocumentRow>) {
    let mut declared = registry(&declaration(copper));
    let original = declared.identities[0].clone();
    declared.identities.clear();
    let mut documents = Vec::new();
    let mut values = Vec::new();
    for (id, file, period, cost, sold, rate, hash, publication) in [
        (
            "before",
            "Before.pdf",
            "2024FY",
            "100",
            "50",
            if copper { "2.000" } else { "2000.000" },
            "11",
            "2025-02-01",
        ),
        (
            "after",
            "After.pdf",
            "2025FY",
            "120",
            "40",
            if copper { "3.000" } else { "3000.000" },
            "22",
            "2026-02-01",
        ),
    ] {
        let mut d = original.clone();
        d.id = id.into();
        d.doc = file.into();
        d.total_period = period.into();
        d.basis = BRIDGE_BASIS.into();
        for t in &mut d.terms {
            t.period = period.into();
        }
        declared.identities.push(d);
        let mut doc = document();
        doc.file = file.into();
        doc.sha256 = hash.repeat(32);
        doc.published_at = publication.into();
        for (quantity, token, unit) in [
            ("cost", cost, "usd_m"),
            ("sold", sold, if copper { "Mlb" } else { "koz" }),
            (
                "rate",
                rate,
                if copper { "usd_per_lb" } else { "usd_per_oz" },
            ),
        ] {
            let mut row = reading(quantity, token, unit);
            row.locator.doc = file.into();
            row.locator.col = period.into();
            row.doc_sha256 = doc.sha256.clone();
            row.basis = BRIDGE_BASIS.into();
            row.period = period.into();
            values.push(row);
        }
        documents.push(doc);
    }
    (declared, values, documents)
}

fn change_token(values: &mut [Accepted], doc: &str, quantity: &str, token: &str) {
    let row = values
        .iter_mut()
        .find(|r| r.locator.doc == doc && r.quantity == quantity)
        .unwrap();
    row.token = token.into();
    row.value = parse_token(token).unwrap();
}

fn frac(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

#[test]
fn cost_volume_bridge_preserves_native_scales_known_answers_and_complete_source_evidence() {
    for copper in [false, true] {
        let (mut declared, mut values, mut documents) = bridge_sources(copper);
        let report = roll_forward::assess(&declared, &values, &documents);
        let original = report.clone();
        let bridge = cost_bridge::assess(&report, "before", "after").unwrap();
        let scale = if copper { 1 } else { 1000 };
        // Independently: (120-100)*(1/50+1/40)/2 = 9/20;
        // (100+120)*(1/40-1/50)/2 = 11/20. No printed rate enters either effect.
        assert_eq!(bridge.bars[0].id, "cost");
        assert_eq!(bridge.bars[0].value.point, frac(9 * scale, 20));
        assert_eq!(bridge.bars[1].id, "sales_volume");
        assert_eq!(bridge.bars[1].value.point, frac(11 * scale, 20));
        assert_eq!(bridge.recomputed_change.point, frac(scale, 1));
        assert_eq!(bridge.printed_rate_gap_change.point, frac(0, 1));
        assert_eq!(bridge.inputs.len(), 6);
        assert_eq!(
            bridge.unit.denominator_unit,
            if copper { "Mlb" } else { "koz" }
        );
        assert_eq!(
            bridge.unit.unit,
            if copper { "usd_per_lb" } else { "usd_per_oz" }
        );
        for bar in &bridge.bars {
            assert!(bar.value.interval.contains(bar.value.point).unwrap());
            assert!(!bar.sign_not_determined);
            assert_eq!(bar.value.inputs.len(), 4);
            for locator in &bar.value.inputs {
                assert!(bridge.inputs.iter().any(|s| s.cell.locator == *locator));
            }
        }
        for source in &bridge.inputs {
            assert!(bridge.to_markdown().contains(&source.document.sha256));
            assert!(bridge.to_json().contains(&source.cell.locator));
        }
        assert!(bridge.to_json().contains("\"identifying\":false"));
        assert!(bridge.to_json().contains("not confidence intervals"));
        assert_eq!(report, original);
        declared.identities.reverse();
        values.reverse();
        documents.reverse();
        let reversed = roll_forward::assess(&declared, &values, &documents);
        assert_eq!(
            bridge,
            cost_bridge::assess(&reversed, "before", "after").unwrap()
        );
    }
}

#[test]
fn printed_endpoint_gaps_remain_separate_from_cost_and_volume_effects() {
    let (declared, mut values, documents) = bridge_sources(true);
    let original = cost_bridge::assess(
        &roll_forward::assess(&declared, &values, &documents),
        "before",
        "after",
    )
    .unwrap();
    change_token(&mut values, "Before.pdf", "rate", "2.010");
    change_token(&mut values, "After.pdf", "rate", "3.020");
    let bridge = cost_bridge::assess(
        &roll_forward::assess(&declared, &values, &documents),
        "before",
        "after",
    )
    .unwrap();
    assert_eq!(bridge.bars, original.bars);
    assert_eq!(bridge.recomputed_change, original.recomputed_change);
    assert_eq!(bridge.endpoints[0].printed_rate_gap.point, frac(1, 100));
    assert_eq!(bridge.endpoints[1].printed_rate_gap.point, frac(1, 50));
    assert_eq!(bridge.printed_rate_gap_change.point, frac(1, 100));
    assert_eq!(bridge.printed_change.point, frac(101, 100));
    assert_ne!(bridge.digest(), original.digest());
    assert_eq!(
        bridge
            .recomputed_change
            .point
            .checked_add(bridge.printed_rate_gap_change.point)
            .unwrap(),
        bridge.printed_change.point
    );
}

#[test]
fn signed_and_zero_costs_keep_exact_closure_and_conservative_corner_enclosures() {
    for sign in [-1, 0, 1] {
        let (declared, mut values, documents) = bridge_sources(true);
        for (doc, cost, rate) in [
            ("Before.pdf", 100 * sign, 2 * sign),
            ("After.pdf", 120 * sign, 3 * sign),
        ] {
            change_token(&mut values, doc, "cost", &cost.to_string());
            change_token(&mut values, doc, "rate", &format!("{rate}.000"));
        }
        let bridge = cost_bridge::assess(
            &roll_forward::assess(&declared, &values, &documents),
            "before",
            "after",
        )
        .unwrap();
        assert_eq!(bridge.recomputed_change.point, frac(sign, 1));
        assert_eq!(bridge.bars[0].value.point, frac(9 * sign, 20));
        assert_eq!(bridge.bars[1].value.point, frac(11 * sign, 20));
        for c0 in [frac(200 * sign - 1, 2), frac(200 * sign + 1, 2)] {
            for c1 in [frac(240 * sign - 1, 2), frac(240 * sign + 1, 2)] {
                for q0 in [frac(99, 2), frac(101, 2)] {
                    for q1 in [frac(79, 2), frac(81, 2)] {
                        let (w0, w1) = (
                            frac(1, 1).checked_div(q0).unwrap(),
                            frac(1, 1).checked_div(q1).unwrap(),
                        );
                        let cost = c1
                            .checked_sub(c0)
                            .unwrap()
                            .checked_mul(w0.checked_add(w1).unwrap())
                            .unwrap()
                            .checked_div(frac(2, 1))
                            .unwrap();
                        let sales = c0
                            .checked_add(c1)
                            .unwrap()
                            .checked_mul(w1.checked_sub(w0).unwrap())
                            .unwrap()
                            .checked_div(frac(2, 1))
                            .unwrap();
                        assert!(bridge.bars[0].value.interval.contains(cost).unwrap());
                        assert!(bridge.bars[1].value.interval.contains(sales).unwrap());
                    }
                }
            }
        }
        if sign == 0 {
            assert!(bridge.bars.iter().all(|b| b.sign_not_determined));
            assert!(bridge.to_markdown().contains("sign not determined"));
        }
    }
}

#[test]
fn cost_bridges_require_matched_explicit_semantics_and_advancing_calendar_support() {
    for changed_basis in [
        "attributable;denominator=sold",
        "subject=synthetic_mine;ownership=attributable:0/1;definition=synthetic@v1;denominator=sold",
        "subject=synthetic_mine;ownership=attributable:2/1;definition=synthetic@v1;denominator=sold",
        "subject=synthetic_mine;ownership=100%;definition=unversioned;denominator=sold",
        "subject=synthetic_mine;ownership=100%;definition=synthetic@v1;denominator=produced",
    ] {
        let (mut declared, mut values, documents) = bridge_sources(true);
        for d in &mut declared.identities {
            d.basis = changed_basis.into();
        }
        for r in &mut values {
            r.basis = changed_basis.into();
        }
        let report = roll_forward::assess(&declared, &values, &documents);
        assert!(report.refused.is_empty());
        assert!(
            cost_bridge::assess(&report, "before", "after").is_err(),
            "{changed_basis}"
        );
    }
    for case in 0..7 {
        let (mut declared, mut values, documents) = bridge_sources(true);
        let after = &mut declared.identities[1];
        match case {
            0 => after.family = "other_metric".into(),
            1 => after.basis = BRIDGE_BASIS.replace("synthetic_mine", "other_mine"),
            2 => after.basis = BRIDGE_BASIS.replace("1/2", "3/4"),
            3 => after.basis = BRIDGE_BASIS.replace("@v1", "@v2"),
            4 => after.total_period = "2024FY".into(),
            5 => after.total_period = "2025H1".into(),
            _ => {
                after.total = "other_rate".into();
                values
                    .iter_mut()
                    .find(|r| r.locator.doc == "After.pdf" && r.quantity == "rate")
                    .unwrap()
                    .quantity = "other_rate".into();
            }
        }
        for term in &mut after.terms {
            term.period = after.total_period.clone();
        }
        for row in values.iter_mut().filter(|r| r.locator.doc == "After.pdf") {
            row.basis = after.basis.clone();
            row.period = after.total_period.clone();
        }
        let report = roll_forward::assess(&declared, &values, &documents);
        assert!(report.refused.is_empty(), "case {case}");
        assert!(
            cost_bridge::assess(&report, "before", "after").is_err(),
            "case {case}"
        );
    }
    let (mut declared, mut values, documents) = bridge_sources(true);
    for (index, period) in ["2025Q1", "2025Q2"].into_iter().enumerate() {
        let d = &mut declared.identities[index];
        d.total_period = period.into();
        for t in &mut d.terms {
            t.period = period.into();
        }
        for r in values.iter_mut().filter(|r| r.locator.doc == d.doc) {
            r.period = period.into();
        }
    }
    let report = roll_forward::assess(&declared, &values, &documents);
    assert!(cost_bridge::assess(&report, "before", "after").is_ok());
    assert!(cost_bridge::assess(&report, "after", "before").is_err());
    assert!(cost_bridge::assess(&report, "before", "before").is_err());
}

#[test]
fn cost_bridge_rebinds_cached_sources_and_replays_connected_component_conflicts() {
    let (mut declared, mut values, documents) = bridge_sources(true);
    let clean = roll_forward::assess(&declared, &values, &documents);
    for case in 0..7 {
        let mut changed = clean.clone();
        match case {
            0 => changed.cells[0].cell.printed = frac(999, 1),
            1 => changed.cells[0].cell.step = frac(1, 100),
            2 => changed.cells[0].source.readers = vec!["one_reader".into()],
            3 => changed.cells[0].source.doc_sha256 = "ff".repeat(32),
            4 => changed.cells[0].source.token = "999".into(),
            5 => {
                changed.cells.pop();
            }
            _ => changed.cells.push(changed.cells[0].clone()),
        }
        assert!(
            cost_bridge::assess(&changed, "before", "after").is_err(),
            "case {case}"
        );
    }
    let mut sum = registry(&format!("{}{SUM}", declaration(true)))
        .identities
        .into_iter()
        .find(|d| d.id == "cost_sum")
        .unwrap();
    sum.doc = "After.pdf".into();
    sum.basis = BRIDGE_BASIS.into();
    declared.identities.push(sum);
    change_token(&mut values, "After.pdf", "sold", "40.000");
    let prototype = values
        .iter()
        .find(|r| r.locator.doc == "After.pdf" && r.quantity == "cost")
        .unwrap()
        .clone();
    for quantity in ["cost_a", "cost_b"] {
        let mut row = prototype.clone();
        row.quantity = quantity.into();
        row.locator.row = quantity.into();
        row.token = "60.20".into();
        row.value = parse_token(&row.token).unwrap();
        values.push(row);
    }
    let mut conflict = roll_forward::assess(&declared, &values, &documents);
    assert!(conflict.refused.is_empty());
    assert!(
        conflict
            .components
            .iter()
            .any(|c| matches!(c.component.outcome, Outcome::Infeasible { .. }))
    );
    assert!(cost_bridge::assess(&conflict, "before", "after").is_err());
    // Forging the cached verdict cannot bypass source rebinding and the joint identity solver.
    for component in &mut conflict.components {
        component.component.outcome = Outcome::Feasible {
            witness: std::collections::BTreeMap::new(),
        };
    }
    assert!(cost_bridge::assess(&conflict, "before", "after").is_err());
}
