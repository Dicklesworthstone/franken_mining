//! Source adapter contracts for the two descriptive Lumwana recovery models (bd-srqc).
//! Synthetic hand answer: h_i=.40+.04i, R_i=91+.4i, i=0..9 gives mean tails=.04044,
//! OLS intercept=.03232 and slope=.014. At h=.62, R=90, processed=6000 kt and produced=33 kt,
//! M-a's gap is 539/155 pp and M-b's is 105/31 pp. Real expectations below are derived outputs
//! from the frozen source-only recovery_census.py oracle; no real historical input tokens are embedded.
#![cfg(test)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use fmine_claim::{Claim, UuTerm};
use fmine_evidence::{EvidenceClass, KnowledgeState};
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{KeyedCell, OwnershipBasis, PublicValue};
use fmine_public::recovery::{self, RecoveryReport};
use fmine_public::sources::{DocumentRow, IssuerProfile, RegisteredSource, SourceKind};
use fmine_public::synthetic;
use fmine_public::transcribe::TranscriptionReport;
use fmine_registry::transcriptions::{Agreement, parse_token};

fn rat(n: i128, d: i128) -> Rat {
    Rat::new(n, d).expect("independent rational")
}

macro_rules! known {
    ($estimate:expr, $n:expr, $d:expr) => {{
        let e = $estimate;
        assert_eq!(e.point, rat($n, $d));
        assert!(e.interval.contains(e.point).expect("ordered bounds"));
        assert!(!e.interval.lo_strict && !e.interval.hi_strict);
    }};
}

/// Reconstruct only synthetic adapter metadata from its public fixture report, for refusal controls.
fn fixture() -> (IssuerProfile, TranscriptionReport, Vec<KeyedCell>) {
    let report = recovery::run_synthetic().expect("synthetic source adapter");
    let mut profile = synthetic::synthetic_profile();
    profile.documents = report
        .documents
        .iter()
        .map(|d| DocumentRow {
            file: d.file.clone(),
            dir: "invented".into(),
            kind: d.kind.clone(),
            url: "https://example.invalid/invented.pdf".into(),
            url_basis: "synthetic test metadata".into(),
            published_at: d.published_at.clone(),
            published_at_basis: d.published_at_basis.clone(),
            pages: 1,
            bytes: 1,
            sha256: d.sha256.clone(),
        })
        .collect();
    let cells = report.quarters[0].inputs.clone();
    let first = &cells[0];
    profile.sources = vec![RegisteredSource {
        file_name: first.locator.split_once(':').expect("file:cell").0.into(),
        kind: SourceKind::MineStats,
        quarter: first.key.reporting_vintage.release.clone(),
        url: "https://example.invalid/invented.xlsx".into(),
        url_basis: "synthetic test metadata".into(),
        bytes: 1,
        sha256_hex: first.key.reporting_vintage.file_sha256.clone(),
    }];
    let mut accepted = report.transcriptions;
    accepted.push(report.ownership_source);
    accepted.sort_by(|a, b| a.locator.cmp(&b.locator));
    let transcriptions = TranscriptionReport {
        registry: "invented test readings".into(),
        documents: report.documents,
        entries: 42,
        agreement: Agreement {
            accepted,
            ..Agreement::default()
        },
        tie: None,
    };
    (profile, transcriptions, cells)
}

fn assessed(p: &IssuerProfile, t: &TranscriptionReport, cells: &[KeyedCell]) -> RecoveryReport {
    recovery::assess(p, Some(t), cells)
        .expect("source adapter")
        .expect("active model")
}

fn remember<T>(definitions: &mut BTreeMap<String, Vec<String>>, claim: &Claim<T>) {
    let mut parents: Vec<_> = claim.parents.iter().map(ToString::to_string).collect();
    parents.sort();
    assert!(
        definitions
            .insert(claim.claim_id.to_string(), parents)
            .is_none()
    );
}

/// Claim IDs contain only canonical ASCII; inspect their stable envelope fields without another JSON dependency.
fn assert_parent_definitions(report: &RecoveryReport) {
    let mut expected = BTreeMap::new();
    remember(&mut expected, &report.ownership);
    remember(&mut expected, &report.fit);
    for row in &report.training {
        remember(&mut expected, &row.head);
        remember(&mut expected, &row.recovery);
    }
    for quarter in &report.quarters {
        for source in &quarter.inputs {
            remember(&mut expected, &source.claim);
        }
        if let Some(row) = &quarter.operating {
            for claim in [&row.head, &row.recovery, &row.processed, &row.produced] {
                remember(&mut expected, claim);
            }
        }
        if let Ok(projection) = &quarter.outcome {
            remember(&mut expected, &projection.constant_tails);
            remember(&mut expected, &projection.grade_dependent_tails);
        }
    }
    let json = report.to_json();
    let mut actual = BTreeMap::new();
    for definition in json.split("\"claim_id\":\"").skip(1) {
        let (id, fields) = definition.split_once('"').expect("quoted Claim ID");
        assert!(id.starts_with("clm_"));
        let mut parents = fields
            .split_once("\"parents\":[")
            .map(|(_, values)| values.split_once(']').expect("closed parent array").0)
            .into_iter()
            .flat_map(|values| values.split(','))
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .expect("quoted parent ID")
                    .to_string()
            })
            .collect::<Vec<_>>();
        parents.sort();
        assert!(
            actual.insert(id.to_string(), parents).is_none(),
            "duplicate definition {id}"
        );
    }
    assert_eq!(
        actual, expected,
        "every typed and raw input must be defined with its parents"
    );
    for (child, parents) in &actual {
        for parent in parents {
            assert!(
                actual.contains_key(parent),
                "undefined {parent}, parent of {child}"
            );
        }
    }
}

#[test]
fn synthetic_exact_models_sources_and_descriptive_output() {
    let report = recovery::run_synthetic().expect("synthetic recovery");
    let fit = &report.fit.value;
    assert_eq!(fit.training_years, (2013..=2022).collect::<Vec<_>>());
    known!(fit.mean_head, 29, 50);
    known!(fit.constant_tails, 1011, 25000);
    known!(fit.intercept, 101, 3125);
    known!(fit.slope, 7, 500);
    assert_eq!(fit.residual_sample_variance_at_printed, rat(44, 29296875));
    assert_eq!(fit.regression_error_variance_at_printed, rat(33, 19531250));
    assert_eq!(report.fit.evidence, EvidenceClass::Assumed);
    assert!(!report.fit.uu_terms.is_empty());
    assert_eq!(report.quarters.len(), 1);
    let quarter = &report.quarters[0];
    assert_eq!(quarter.period.code(), "2025Q1");
    assert_eq!(quarter.release, "2025Q1");
    assert_eq!(quarter.inputs.len(), 4);
    assert!(
        quarter
            .inputs
            .iter()
            .all(|c| c.key.ownership_basis == OwnershipBasis::Unprinted)
    );
    let projection = quarter.outcome.as_ref().expect("both model projections");
    let (a, b) = (
        &projection.constant_tails.value,
        &projection.grade_dependent_tails.value,
    );
    known!(a.expected_recovery_percent, 14489, 155);
    known!(a.gap_pp, 539, 155);
    known!(a.copper_difference_kt, 1617, 1250);
    known!(a.fraction_of_reported_output, 49, 1250);
    known!(b.expected_recovery_percent, 2895, 31);
    known!(b.gap_pp, 105, 31);
    known!(b.copper_difference_kt, 63, 50);
    known!(b.fraction_of_reported_output, 21, 550);
    for model in [
        &projection.constant_tails,
        &projection.grade_dependent_tails,
    ] {
        assert_eq!(model.evidence, EvidenceClass::Assumed);
        assert!(model.parents.contains(&report.fit.claim_id));
        assert!(quarter.inputs.iter().all(|input| {
            input
                .claim
                .anchors
                .iter()
                .all(|a| model.anchors.contains(a))
        }));
        assert!(
            report
                .ownership
                .anchors
                .iter()
                .all(|a| model.anchors.contains(a))
        );
    }
    let json = report.to_json();
    assert_parent_definitions(&report);
    for text in [
        "fmine.recovery.gap.v1",
        "\"descriptive_only\":true",
        "\"identifying\":false",
        "uncalibrated_descriptive_model",
        "not confidence or prediction intervals",
        "monetary_downside\":\"unquantified",
        "\"in\":[",
        "\"sha256\":",
    ] {
        assert!(json.contains(text), "missing {text}");
    }
    let again = recovery::run_synthetic().expect("repeat");
    assert_eq!(report.digest(), again.digest());
    assert_eq!(json, again.to_json());
}

#[test]
fn missing_history_reader_or_source_evidence_refuses() {
    let (p, t, cells) = fixture();
    assert!(
        recovery::assess(&p, None, &cells)
            .expect("ordinary assurance remains usable")
            .is_none()
    );
    let target = t
        .agreement
        .accepted
        .iter()
        .position(|a| a.quantity.ends_with("2022_head_grade_pct"))
        .expect("annual row");
    let mut partial = t.clone();
    partial.agreement.accepted.remove(target);
    assert_eq!(
        recovery::assess(&p, Some(&partial), &cells)
            .expect_err("no partial training")
            .code,
        "FMINE-REF-202"
    );
    let mut single = t.clone();
    single.agreement.accepted[target].readers.truncate(1);
    assert_eq!(
        recovery::assess(&p, Some(&single), &cells)
            .expect_err("two readers required")
            .code,
        "FMINE-REF-201"
    );
    let mut digest = t.clone();
    digest.agreement.accepted[target].doc_sha256 = "f".repeat(64);
    assert_eq!(
        recovery::assess(&p, Some(&digest), &cells)
            .expect_err("checked source hash")
            .code,
        "FMINE-REF-505"
    );
    let mut unit = t.clone();
    unit.agreement.accepted[target].unit = "g/t".into();
    assert_eq!(
        recovery::assess(&p, Some(&unit), &cells)
            .expect_err("explicit annual percent unit")
            .code,
        "FMINE-REF-102"
    );
    let mut duplicate = t.clone();
    duplicate
        .agreement
        .accepted
        .push(t.agreement.accepted[target].clone());
    assert_eq!(
        recovery::assess(&p, Some(&duplicate), &cells)
            .expect_err("unique historical source")
            .code,
        "FMINE-REF-202"
    );
    let mut uncited = t.clone();
    uncited.agreement.accepted[target].locator.page = 0;
    assert_eq!(
        recovery::assess(&p, Some(&uncited), &cells)
            .expect_err("historical citation is required")
            .code,
        "FMINE-REF-201"
    );
    // A coherent changed reading must flow through fitting and the report commitment.
    let baseline = assessed(&p, &t, &cells);
    let mut changed = t.clone();
    changed.agreement.accepted[target].token = "0.77".into();
    changed.agreement.accepted[target].value =
        parse_token("0.77").expect("invented changed head grade");
    let changed = assessed(&p, &changed, &cells);
    assert_ne!(
        baseline.fit.value.intercept.point,
        changed.fit.value.intercept.point
    );
    assert_ne!(
        baseline.fit.value.slope.point,
        changed.fit.value.slope.point
    );
    assert_ne!(baseline.digest(), changed.digest());
}

#[test]
fn quarterly_refusals_preserve_population_and_first_release() {
    let (p, t, cells) = fixture();
    let head = cells
        .iter()
        .position(|c| c.key.line.label.ends_with(":processed"))
        .expect("head");
    let mut missing = cells.clone();
    missing.remove(head);
    let mut later = cells.clone();
    for cell in &mut later {
        cell.key.reporting_vintage.release = "2025Q2".into();
    }
    missing.extend(later);
    let out = assessed(&p, &t, &missing);
    assert_eq!(out.quarters.len(), 1);
    assert_eq!(out.quarters[0].release, "2025Q1");
    assert_eq!(out.quarters[0].inputs.len(), 3);
    let refusal = out.quarters[0]
        .outcome
        .as_ref()
        .expect_err("later comparison must not repair original");
    assert_eq!(refusal.code, "FMINE-REF-202");
    assert!(refusal.what.contains("HeadGradePercent"));
    let mut wrong_unit = cells.clone();
    wrong_unit[head].key.unit = "g/t".into();
    assert_eq!(
        assessed(&p, &t, &wrong_unit).quarters[0]
            .outcome
            .as_ref()
            .expect_err("units")
            .code,
        "FMINE-REF-105"
    );
    let mut wrong_hash = cells.clone();
    wrong_hash[head].claim.anchors[0].artifact_sha256[0] ^= 1;
    assert_eq!(
        assessed(&p, &t, &wrong_hash).quarters[0]
            .outcome
            .as_ref()
            .expect_err("source hash")
            .code,
        "FMINE-REF-505"
    );
    let mut zero = cells.clone();
    zero[head].claim.value = PublicValue::Number {
        stored: "0".into(),
        displayed: Ok("0.00".into()),
        under_status: None,
    };
    assert_eq!(
        assessed(&p, &t, &zero).quarters[0]
            .outcome
            .as_ref()
            .expect_err("positive head grade")
            .code,
        "FMINE-REF-109"
    );
    assert_ne!(out.digest(), assessed(&p, &t, &cells).digest());
}

#[test]
fn full_site_context_does_not_rewrite_source_keys_and_order_is_stable() {
    let (p, mut t, mut cells) = fixture();
    let original = cells.clone();
    let out = assessed(&p, &t, &cells);
    assert_eq!(out.quarters[0].inputs, original);
    assert!(
        out.quarters[0]
            .inputs
            .iter()
            .all(|c| c.printed_basis == OwnershipBasis::Unprinted)
    );
    t.agreement.accepted.reverse();
    cells.reverse();
    let reordered = assessed(&p, &t, &cells);
    assert_eq!(out.digest(), reordered.digest());
    assert_eq!(out.to_json(), reordered.to_json());
    let ownership = t
        .agreement
        .accepted
        .iter_mut()
        .find(|a| a.quantity == recovery::OWNERSHIP_QUANTITY)
        .expect("ownership context");
    ownership.locator.row = "Lumwana wholly owned".into();
    assert_eq!(
        recovery::assess(&p, Some(&t), &cells)
            .expect_err("printed numeric ownership required")
            .code,
        "FMINE-REF-107"
    );
}

#[test]
fn ownership_context_matches_exact_subject_table_and_column() {
    let (p, t, cells) = fixture();
    let target = t
        .agreement
        .accepted
        .iter()
        .position(|a| a.quantity == recovery::OWNERSHIP_QUANTITY)
        .expect("ownership context");
    for (field, value) in [
        ("row", "Not Lumwana (100%)"),
        ("row", "Lumwana (100%) elsewhere"),
        ("table", "Unrelated ownership table"),
        ("col", "Other interest (%)"),
    ] {
        let mut changed = t.clone();
        let locator = &mut changed.agreement.accepted[target].locator;
        match field {
            "row" => locator.row = value.into(),
            "table" => locator.table = value.into(),
            "col" => locator.col = value.into(),
            _ => unreachable!(),
        }
        assert_eq!(
            recovery::assess(&p, Some(&changed), &cells)
                .expect_err("exact ownership source labels")
                .code,
            "FMINE-REF-107",
            "{field}: {value}"
        );
    }
}

#[test]
fn direct_api_rejects_malformed_through_before_corpus_io() {
    let unavailable =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("unavailable_recovery_workspace");
    let (mut p, t, cells) = fixture();
    for through in [
        "2026", "2026Q0", "2026Q5", "202AQ1", "2026Q1x", "2026q1", " 2026Q1", "2026Q1 ",
    ] {
        assert_eq!(
            recovery::run_public(&unavailable, "barrick", Some(through))
                .expect_err("syntax precedes source I/O")
                .code,
            "FMINE-REF-107",
            "{through:?}"
        );
        p.through = Some(through.into());
        assert_eq!(
            recovery::assess(&p, Some(&t), &cells)
                .expect_err("active direct profile bound")
                .code,
            "FMINE-REF-107",
            "{through:?}"
        );
        assert!(
            recovery::assess(&p, None, &cells)
                .expect("no registered recovery inputs")
                .is_none()
        );
    }
}

#[test]
fn inherited_quantified_uu_survives_model_and_json() {
    let (p, t, mut cells) = fixture();
    let risk = UuTerm {
        scenario_id: "synthetic-source-risk".into(),
        description: "Invented, explicitly quantified source downside".into(),
        downside_exposure_usd_m: 12.5,
        trigger_indicator: "Synthetic source scenario".into(),
    };
    cells[0].claim.uu_terms.push(risk.clone());
    let report = assessed(&p, &t, &cells);
    let projection = report.quarters[0].outcome.as_ref().expect("models");
    for model in [
        &projection.constant_tails,
        &projection.grade_dependent_tails,
    ] {
        assert!(model.uu_terms.contains(&risk));
    }
    let json = report.to_json();
    let occurrences = json
        .split("\"scenario_id\":\"synthetic-source-risk\"")
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(
        occurrences.len(),
        3,
        "one operating Claim and both projections retain the inherited amount"
    );
    for tail in occurrences {
        let term = tail.split_once('}').expect("closed inherited amount").0;
        assert!(term.contains("\"monetary_downside\":\"source_estimate\""));
        assert!(term.contains("\"downside_exposure_usd_m\":{\"v\":12.5"));
        assert!(term.contains("\"rule\":\"recovery.inherited_uu\""));
        assert!(term.contains("\"rules_version\":\"public-recovery/1\""));
        assert!(term.contains("\"in\":["));
    }
    assert!(
        json.contains("\"monetary_downside\":\"unquantified\""),
        "the model's separate omitted-mechanism term remains unquantified"
    );
    assert_parent_definitions(&report);
}

#[test]
#[ignore = "requires the verified public PDFs/workbooks and local two-reader transcription registry"]
fn real_lumwana_matches_independent_source_oracle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report =
        recovery::run_public(&root, "barrick", Some("2026Q2")).expect("registered source inputs");
    assert!(!report.synthetic);
    assert_eq!(report.training.len(), 10);
    assert_eq!(report.transcriptions.len(), 20);
    assert_eq!(report.documents.len(), 2);
    assert_parent_definitions(&report);
    // NL-2 orders issuer assertions below modelled evidence: derivation cannot promote the inputs.
    // Estimated knowledge and the explicit calibration field retain the model interpretation.
    assert_eq!(report.fit.evidence, EvidenceClass::ISSUER);
    assert_eq!(report.fit.state, KnowledgeState::Estimated);
    assert!(report.to_json().contains("uncalibrated_descriptive_model"));
    known!(report.fit.value.mean_head, 11, 20);
    known!(report.fit.value.constant_tails, 9073, 250000);
    known!(report.fit.value.intercept, 1255041, 110500000);
    known!(report.fit.value.slope, 10019, 221000);
    let sse = rat(423083529, 5525000000000);
    assert_eq!(
        report.fit.value.residual_sample_variance_at_printed,
        sse.checked_div(rat(9, 1)).expect("n-1")
    );
    assert_eq!(
        report.fit.value.regression_error_variance_at_printed,
        sse.checked_div(rat(8, 1)).expect("n-2")
    );
    for (sd, independent) in [
        (
            report.fit.value.residual_sample_sd_at_printed,
            0.002_916_927_690_108_424,
        ),
        (
            report.fit.value.regression_error_sd_at_printed,
            0.003_093_869_024_859_718_5,
        ),
    ] {
        assert!((sd.approx - independent).abs() < 1e-17);
        assert!(sd.enclosure[0] <= independent && independent <= sd.enclosure[1]);
    }
    // These derived rational gaps were frozen from PDF + raw XLSX reads before inspecting Rust.
    let expected = [
        ("2023Q1", 3277, 950, 135297, 24700),
        ("2023Q2", -323, 1250, 107709, 552500),
        ("2023Q3", 3302, 1375, 3302, 1375),
        ("2023Q4", 8477, 1350, 140627, 22100),
        ("2024Q1", 3227, 1025, 1063832, 226525),
        ("2024Q2", 7802, 1125, 219413, 27625),
        ("2024Q3", 2852, 1325, 680387, 292825),
        ("2024Q4", 3352, 1775, 340032, 392275),
        ("2025Q1", 3752, 1425, 259699, 104975),
        ("2025Q2", 4327, 1675, 655697, 370175),
        ("2025Q3", 4127, 1650, 424363, 243100),
        ("2025Q4", 5552, 1625, 976517, 359125),
        ("2026Q1", 4202, 1475, 828452, 325975),
        ("2026Q2", 5327, 1600, 1903679, 707200),
    ];
    assert_eq!(report.quarters.len(), expected.len());
    let mut unknown_sign_a = Vec::new();
    let mut unknown_sign_b = Vec::new();
    for (quarter, (period, an, ad, bn, bd)) in report.quarters.iter().zip(expected) {
        assert_eq!(quarter.period.code(), period);
        assert_eq!(
            quarter.release,
            if period.starts_with("2023") {
                period.replacen("2023", "2024", 1)
            } else {
                period.into()
            }
        );
        assert_eq!(quarter.inputs.len(), 4);
        let projection = quarter.outcome.as_ref().expect("source-supported quarter");
        known!(projection.constant_tails.value.gap_pp, an, ad);
        known!(projection.grade_dependent_tails.value.gap_pp, bn, bd);
        for model in [
            &projection.constant_tails,
            &projection.grade_dependent_tails,
        ] {
            assert_eq!(model.evidence, EvidenceClass::ISSUER);
            assert_eq!(model.state, KnowledgeState::Estimated);
            assert!(report.fit.anchors.iter().all(|a| model.anchors.contains(a)));
            assert!(
                quarter.inputs.iter().all(|c| c
                    .claim
                    .anchors
                    .iter()
                    .all(|a| model.anchors.contains(a)))
            );
        }
        if projection
            .constant_tails
            .value
            .gap_pp
            .interval
            .contains(rat(0, 1))
            .expect("bounds")
        {
            unknown_sign_a.push(period);
        }
        if projection
            .grade_dependent_tails
            .value
            .gap_pp
            .interval
            .contains(rat(0, 1))
            .expect("bounds")
        {
            unknown_sign_b.push(period);
        }
    }
    assert_eq!(unknown_sign_a, ["2023Q2"]);
    assert_eq!(unknown_sign_b, ["2023Q2", "2024Q4"]);
    let last = &report
        .quarters
        .last()
        .expect("2026Q2")
        .outcome
        .as_ref()
        .expect("model")
        .grade_dependent_tails
        .value;
    known!(last.copper_difference_kt, 6611477167, 5525000000);
    known!(last.fraction_of_reported_output, 6611477167, 226525000000);
    let again = recovery::run_public(&root, "barrick", Some("2026Q2")).expect("repeat");
    assert_eq!(report.digest(), again.digest());
    assert_eq!(report.to_json(), again.to_json());
}
