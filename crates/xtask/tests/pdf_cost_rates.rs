//! Constructed PDF cost-rate identities through the registry, source boundary and exact joint solver.
//! These known answers do not authenticate a real filing or establish an accounting definition.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use fmine_metrics::roll_forward::{self, TableReport};
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
