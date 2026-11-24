//! Exact known answers, source-binding and no-laundering controls for capital return.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use super::*;
use fmine_claim::{Confidentiality, SpatialSupport, UuTerm};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::EvidenceClass;
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};

const FIXTURE: &str = include_str!("../../../../../tests/metrics/capital_return.toml");
const ZERO: ExactQty = ExactQty::from_integer(0);

fn q(n: i128, d: i128) -> ExactQty {
    ExactQty::new(n, d).unwrap()
}

fn declared(text: &str) -> Result<CapitalReturnReceipt> {
    worksheet::evaluate(
        text,
        SourceAnchor::new(
            "synthetic/return.toml",
            sha256(text.as_bytes()).0,
            "worksheet",
        ),
    )
}

fn inputs() -> Vec<Claim<Input>> {
    declared(FIXTURE).unwrap().inputs().to_vec()
}

fn rebind(claim: &Claim<Input>, value: Input) -> Claim<Input> {
    let tag = value.canonical_tag();
    let additivity = value.additivity();
    Claim::new(
        INPUT_METRIC,
        value,
        &tag,
        claim.representation.clone(),
        claim.support,
        additivity,
        claim.evidence,
        claim.epistemic,
        claim.state,
        claim.confidentiality.clone(),
        claim.uu_terms.clone(),
        claim.parents.clone(),
        claim.anchors.clone(),
    )
}

fn change(rows: &mut [Claim<Input>], role: &str, f: impl FnOnce(&mut Input)) {
    let row = rows
        .iter_mut()
        .find(|c| c.value.value.role() == role)
        .unwrap();
    let mut value = row.value.clone();
    f(&mut value);
    *row = rebind(row, value);
}

fn basis(rows: &mut Vec<Claim<Input>>, basis: CapitalBasis) {
    change(rows, "definition", |input| {
        let InputValue::Definition(definition) = &mut input.value else {
            panic!("definition");
        };
        definition.capital_basis = basis;
    });
    rows.retain(|c| {
        matches!(c.value.value.role(), "definition" | "earnings")
            || basis.roles().contains(&c.value.value.role())
    });
}

fn set_amount(input: &mut Input, value: ExactQty, step: ExactQty) {
    let amount = match &mut input.value {
        InputValue::Earnings { amount, .. }
        | InputValue::Opening { amount, .. }
        | InputValue::Closing { amount, .. }
        | InputValue::Average { amount, .. } => amount,
        _ => panic!("monetary input"),
    };
    *amount = Amount {
        value,
        rounding_step: step,
    };
}

#[test]
fn named_capital_bases_have_distinct_exact_answers_and_explicit_annualization() {
    let base = declared(FIXTURE).unwrap();
    assert_eq!(base.summary().capital_employed.point, q(100, 1));
    assert_eq!(base.summary().period_return_percent.point, q(12, 1));
    assert_eq!(
        base.summary().annualized_return_percent.unwrap().point,
        q(24, 1)
    );
    assert_eq!(
        base.summary().period_return_percent.bounds,
        closed(q(12, 1), q(12, 1))
    );
    for (selected, expected) in [
        (CapitalBasis::Opening, q(15, 1)),
        (CapitalBasis::Closing, q(10, 1)),
    ] {
        let mut rows = inputs();
        basis(&mut rows, selected);
        let result = evaluate(&rows).unwrap();
        assert_eq!(result.summary().period_return_percent.point, expected);
        assert_ne!(result.digest(), base.digest());
    }
    let mut rows = inputs();
    change(&mut rows, "closing", |c| {
        c.value = InputValue::Average {
            period: c.scope.period,
            amount: Amount {
                value: q(96, 1),
                rounding_step: ZERO,
            },
        }
    });
    basis(&mut rows, CapitalBasis::ReportedAverage);
    change(&mut rows, "definition", |c| {
        let InputValue::Definition(d) = &mut c.value else {
            panic!("definition");
        };
        d.annualization = Annualization::None;
    });
    let result = evaluate(&rows).unwrap();
    assert_eq!(result.summary().period_return_percent.point, q(25, 2));
    assert_eq!(result.summary().annualized_return_percent, None);
    assert_eq!(result.capital_claim().parents.len(), 2);
    assert!(
        result
            .claim()
            .parents
            .contains(&result.capital_claim().claim_id)
    );
    for input in result.inputs() {
        assert!(result.claim().parents.contains(&input.claim_id));
    }
    assert_eq!(result.claim().additivity, AdditivityClass::NonAdditive);
}

#[test]
fn signed_earnings_rounding_enclosures_cover_all_corners_and_keep_zero() {
    let mut rows = inputs();
    basis(&mut rows, CapitalBasis::Closing);
    change(&mut rows, "earnings", |c| set_amount(c, q(-7, 1), q(2, 1)));
    change(&mut rows, "closing", |c| set_amount(c, q(100, 1), q(20, 1)));
    let result = evaluate(&rows).unwrap();
    assert_eq!(result.summary().period_return_percent.point, q(-7, 1));
    assert_eq!(
        result.summary().period_return_percent.bounds,
        closed(q(-80, 9), q(-60, 11))
    );
    for n in [-8, -7, -6] {
        for d in [90, 100, 110] {
            assert!(
                result
                    .summary()
                    .period_return_percent
                    .bounds
                    .contains(q(100 * n, d))
                    .unwrap()
            );
        }
    }
    change(&mut rows, "earnings", |c| set_amount(c, ZERO, ZERO));
    assert_eq!(
        evaluate(&rows)
            .unwrap()
            .summary()
            .period_return_percent
            .bounds,
        closed(ZERO, ZERO)
    );
    change(&mut rows, "earnings", |c| set_amount(c, ZERO, q(2, 1)));
    assert_eq!(
        evaluate(&rows)
            .unwrap()
            .summary()
            .period_return_percent
            .bounds,
        closed(q(-10, 9), q(10, 9))
    );
}

#[test]
fn nonpositive_or_zero_crossing_selected_capital_and_excluded_issuer_returns_refuse() {
    for (value, step) in [(ZERO, ZERO), (q(-100, 1), ZERO), (q(1, 1), q(2, 1))] {
        let mut rows = inputs();
        basis(&mut rows, CapitalBasis::Closing);
        change(&mut rows, "closing", |c| set_amount(c, value, step));
        assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-104");
    }
    let mut rows = inputs();
    change(&mut rows, "opening", |c| set_amount(c, ZERO, ZERO));
    assert_eq!(
        evaluate(&rows)
            .unwrap()
            .summary()
            .period_return_percent
            .point,
        q(20, 1)
    );
    change(&mut rows, "definition", |c| {
        let InputValue::Definition(d) = &mut c.value else {
            panic!("definition");
        };
        d.applicability = Applicability::Excluded {
            reason: "non-producing development project".into(),
        };
    });
    let refusal = evaluate(&rows).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-208");
    assert!(refusal.what.contains("non-producing"));
}

#[test]
fn dates_periods_scope_currency_and_ownership_cannot_be_substituted() {
    let mut rows = inputs();
    change(&mut rows, "opening", |c| {
        let InputValue::Opening { as_of, .. } = &mut c.value else {
            panic!("opening");
        };
        as_of.day = 30;
    });
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-102");
    let mut rows = inputs();
    change(&mut rows, "earnings", |c| {
        let InputValue::Earnings { period, .. } = &mut c.value else {
            panic!("earnings");
        };
        period.last_month = 3;
    });
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-102");
    for kind in 0..5 {
        let mut rows = inputs();
        change(&mut rows, "closing", |c| match kind {
            0 => c.scope.ownership = Ownership::Attributable { share: q(3, 5) },
            1 => c.scope.currency = Currency::CAD,
            2 => c.scope.monetary_scale = 1,
            3 => c.scope.perimeter = "different operations".into(),
            _ => c.scope.accounting_basis = "purchase price allocation".into(),
        });
        assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-105");
    }
    let mut rows = inputs();
    change(&mut rows, "definition", |c| {
        let InputValue::Definition(d) = &mut c.value else {
            panic!("definition");
        };
        d.capital_basis = CapitalBasis::Closing;
    });
    assert_eq!(
        evaluate(&rows).unwrap_err().code,
        "FMINE-REF-105",
        "unused opening capital must not disappear"
    );
}

#[test]
fn claims_keep_ancestry_evidence_and_exact_replay_and_reject_mutations() {
    let original = declared(FIXTURE).unwrap();
    assert_eq!(original.claim().evidence, EvidenceClass::Assumed);
    assert_eq!(
        original.claim().confidentiality,
        Confidentiality::internal()
    );
    let mut reversed = original.inputs().to_vec();
    reversed.reverse();
    assert_eq!(evaluate(&reversed).unwrap().to_json(), original.to_json());
    let mut rows = inputs();
    rows[0].value.scope.subject = "mutated without a new Claim".into();
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-507");
    let mut rows = inputs();
    rows[0].support = SpatialSupport::PointCore;
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-102");
    let mut rows = inputs();
    rows[0].stale = true;
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-502");
    let mut rows = inputs();
    rows[0].uu_terms.push(UuTerm {
        scenario_id: "reclassification".into(),
        description: "capital classification may change".into(),
        downside_exposure_usd_m: 1.25,
        trigger_indicator: "restated filing".into(),
    });
    let enriched = evaluate(&rows).unwrap();
    assert_eq!(enriched.claim().uu_terms.len(), 1);
    assert_ne!(enriched.digest(), original.digest());
    assert!(original.to_json().contains("canonical_body_hex"));
    assert!(
        original
            .to_markdown()
            .contains(&original.capital_claim().claim_id.to_string())
    );
    let changed = declared(&FIXTURE.replace("amount = \"12\"", "amount = \"13\"")).unwrap();
    assert_eq!(changed.summary().period_return_percent.point, q(13, 1));
    assert_ne!(changed.digest(), original.digest());
}

#[test]
fn worksheet_rejects_ambiguous_definitions_overrides_and_wrong_hashes() {
    for (old, new) in [
        ("tax_basis = \"pre_tax\"", "tax_basis = \"standard_ifrs\""),
        ("annualization = \"explicit\"", "annualization = \"none\""),
        ("ownership = \"full\"", "ownership = \"attributable\""),
        (
            "capital_basis = \"opening_closing_mean\"",
            "capital_basis = \"weighted_average\"",
        ),
        ("rounding_step = \"0\"", "rounding_step = \"-1\""),
    ] {
        assert!(declared(&FIXTURE.replace(old, new)).is_err(), "{new}");
    }
    assert!(declared(&format!("evidence = \"Measured\"\n{FIXTURE}")).is_err());
    let source = SourceAnchor::new("synthetic/return.toml", [1; 32], "worksheet");
    assert_eq!(
        worksheet::evaluate(FIXTURE, source).unwrap_err().code,
        "FMINE-REF-505"
    );
    let mut rows = inputs();
    basis(&mut rows, CapitalBasis::Closing);
    change(&mut rows, "earnings", |c| {
        set_amount(c, q(i128::MAX, 1), ZERO)
    });
    change(&mut rows, "closing", |c| set_amount(c, ONE, ZERO));
    assert_eq!(evaluate(&rows).unwrap_err().code, "FMINE-REF-207");
}

fn filing() -> (DocumentRow, Vec<Accepted>) {
    let document = DocumentRow {
        file: "invented.pdf".into(),
        dir: "synthetic".into(),
        kind: "mda".into(),
        url: "https://example.invalid/invented.pdf".into(),
        url_basis: "synthetic fixture".into(),
        published_at: "2026-08-01".into(),
        published_at_basis: "FACT: invented test release".into(),
        pages: 3,
        bytes: 42,
        sha256: "12".repeat(32),
    };
    let common = "subject=synthetic_mine;ownership=full;perimeter=continuing;accounting_basis=historical_cost;currency=USD;monetary_scale=1000000;definition=issuer_return;version=1;earnings_basis=adjusted operating earnings;tax_basis=after_tax;capital_employed_basis=net operating assets;capital_basis=opening_closing_mean;annualization=explicit;annualization_factor=2;annualization_basis=half_year_times_two;applicability=applicable;definition_page=2;definition_table=Definitions;definition_row=Capital return;definition_col=Basis";
    let rows = [
        ("earnings", "12.0", None),
        ("opening", "80.0", Some("2025-12-31")),
        ("closing", "120.0", Some("2026-06-30")),
    ]
    .into_iter()
    .map(|(role, token, date)| Accepted {
        locator: Locator {
            doc: document.file.clone(),
            page: 1,
            table: "Return".into(),
            row: role.into(),
            col: "H1".into(),
        },
        doc_sha256: document.sha256.clone(),
        token: token.into(),
        value: parse_token(token).unwrap(),
        quantity: format!("roce_test_{role}"),
        unit: "USD_m".into(),
        basis: date.map_or_else(|| common.into(), |date| format!("{common};snapshot={date}")),
        period: "2026H1".into(),
        readers: vec!["reader_a".into(), "reader_b".into()],
    })
    .collect();
    (document, rows)
}

#[test]
fn agreed_filing_readings_retain_precision_definition_citations_and_readers() {
    let (document, rows) = filing();
    let period = Period {
        year: 2026,
        first_month: 1,
        last_month: 6,
    };
    let bound = source::bind("synthetic_issuer", "roce_test", period, &document, &rows).unwrap();
    let result = evaluate(&bound).unwrap();
    assert_eq!(result.summary().period_return_percent.point, q(12, 1));
    assert_eq!(
        result.summary().period_return_percent.bounds,
        closed(q(23900, 2001), q(24100, 1999))
    );
    assert_eq!(result.claim().evidence, EvidenceClass::ISSUER);
    assert!(result.claim().confidentiality.is_public());
    assert_eq!(result.claim().anchors.len(), 4);
    assert!(result.to_json().contains("reader_a"));
    assert!(result.to_json().contains("adjusted operating earnings"));
    let mut changed = rows.clone();
    changed[0].readers[1] = "reader_c".into();
    let changed = evaluate(
        &source::bind("synthetic_issuer", "roce_test", period, &document, &changed).unwrap(),
    )
    .unwrap();
    assert_eq!(
        changed.summary().period_return_percent,
        result.summary().period_return_percent
    );
    assert_ne!(changed.digest(), result.digest());
}

#[test]
fn filing_binding_refuses_source_mutation_reader_shortfalls_and_mismatched_definitions() {
    let (document, rows) = filing();
    let period = Period {
        year: 2026,
        first_month: 1,
        last_month: 6,
    };
    for kind in 0..6 {
        let mut changed = rows.clone();
        match kind {
            0 => changed[0].token = "13.0".into(),
            1 => changed[0].readers.truncate(1),
            2 => {
                changed[0].basis = changed[0]
                    .basis
                    .replace("tax_basis=after_tax", "tax_basis=pre_tax")
            }
            3 => changed[0].unit = "CAD_m".into(),
            4 => {
                changed[0].basis = changed[0]
                    .basis
                    .replace("definition_page=2", "definition_page=4")
            }
            _ => changed[0].doc_sha256 = "34".repeat(32),
        }
        assert!(
            source::bind("synthetic_issuer", "roce_test", period, &document, &changed).is_err()
        );
    }
    let mut future = document.clone();
    future.published_at = "2026-05-01".into();
    assert_eq!(
        source::bind("synthetic_issuer", "roce_test", period, &future, &rows)
            .unwrap_err()
            .code,
        "FMINE-REF-108"
    );
}

#[test]
fn filing_exclusion_preserves_its_reason_without_relaxing_source_verification() {
    let (document, mut rows) = filing();
    let period = Period {
        year: 2026,
        first_month: 1,
        last_month: 6,
    };
    for row in &mut rows {
        row.basis = row.basis.replace(
            "applicability=applicable",
            "applicability=excluded;exclusion_reason=development project not producing",
        );
    }
    rows[0].token = "10.0 - 14.0".into();
    rows[0].value = parse_token(&rows[0].token).unwrap();
    let refusal =
        source::bind("synthetic_issuer", "roce_test", period, &document, &rows).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-208");
    assert!(refusal.what.contains("development project not producing"));
    rows[0].token = "—".into();
    assert_eq!(
        source::bind("synthetic_issuer", "roce_test", period, &document, &rows)
            .unwrap_err()
            .code,
        "FMINE-REF-507"
    );
}

#[test]
fn markdown_retains_derived_claim_metadata_and_escapes_source_text() {
    let mut rows = inputs();
    for row in &mut rows {
        let mut value = row.value.clone();
        value.scope.subject = "<script>alert(`x`)</script> | [invented](unsafe) *mine*".into();
        *row = rebind(row, value);
    }
    let result = evaluate(&rows).unwrap();
    let markdown = result.to_markdown();
    let visible = markdown.split("## Derived Claim receipts").next().unwrap();
    assert!(!visible.contains("<script>"));
    assert!(visible.contains("&lt;script&gt;alert(\\`x\\`)&lt;/script&gt;"));
    assert!(visible.contains("\\| \\[invented\\](unsafe) \\*mine\\*"));
    assert!(markdown.contains(&claim_json(
        result.capital_claim(),
        &capital_json(&result.capital_claim().value),
        false
    )));
    assert!(markdown.contains(&claim_json(
        result.claim(),
        &summary_json(result.summary()),
        false
    )));
    for parent in &result.claim().parents {
        assert!(markdown.contains(&parent.to_string()));
    }
}
