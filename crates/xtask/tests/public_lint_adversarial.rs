//! Adversarial known answers for the bounded cited-equality surface; all source assertions are invented.
use fmine_core::hash::Digest32;
use fmine_evidence::KnowledgeState;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::key::{
    KeyedCell, LineKey, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue, Scope,
    ShareSource, Vintage, classify, public_claim,
};
use fmine_public::lint::{Verdict, lint_cited, lint_discover};

fn source(subject: &str, release: &str, basis: OwnershipBasis) -> KeyedCell {
    source_line(subject, release, basis, "summary:aisc_usd_per_oz")
}

#[allow(clippy::expect_used)] // A malformed invented workbook must fail the fixture immediately.
fn source_line(subject: &str, release: &str, basis: OwnershipBasis, line: &str) -> KeyedCell {
    let mut key = PublicKey {
        issuer: "synthetic".into(),
        subject: subject.into(),
        valid_period: Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: release.into(),
            file_sha256: String::new(),
        },
        ownership_basis: basis,
        scope: Scope::Site,
        unit: "usd_per_oz".into(),
        line: LineKey {
            label: line.into(),
            occurrence: 0,
        },
    };
    let file = format!("synthetic-{release}.xlsx");
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "QTD".into(),
        hidden: false,
        cells: vec![("A1".into(), MinimalCell::Number(1479.0))],
    }]);
    let workbook = Workbook::from_bytes(&file, &bytes).expect("invented source workbook");
    key.reporting_vintage.file_sha256 = Digest32(workbook.sha256).to_hex();
    let value = classify(workbook.sheets[0].cell(1, 1), None).expect("source value");
    public_claim(&workbook, "QTD", "A1", key, value)
}

fn draft(claim: &KeyedCell, ownership: &str, amount: &str) -> String {
    format!(
        "Site A Q1 2030 {ownership} AISC per ounce sold is [{amount}](fmine:{}).",
        claim.claim.claim_id
    )
}

fn no_verified(draft: &str, claims: &[KeyedCell]) {
    let report = lint_cited(draft, claims);
    assert!(
        !report.findings.is_empty(),
        "numeric finding must remain visible: {draft}"
    );
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.verdict == Verdict::Verified),
        "unsupported or contradicted equality verified: {draft}\n{}",
        report.to_json()
    );
}

#[test]
fn old_claim_identity_cannot_verify_changed_values_or_semantics() {
    let original = source("Site A", "2030Q1", OwnershipBasis::Consolidated100);
    original.claim.check_id().expect("canonical workbook Claim");
    let original_id = original.claim.claim_id.to_string();
    let positive = "Site A Q1 2030 100% basis AISC per ounce sold is US$1,479/oz.";
    for report in [
        lint_cited(
            &draft(&original, "100% basis", "US$1,479/oz"),
            std::slice::from_ref(&original),
        ),
        lint_discover(positive, std::slice::from_ref(&original)),
    ] {
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].verdict,
            Verdict::Verified,
            "{}",
            report.to_json()
        );
    }
    for mutation in 0..6 {
        let mut changed = original.clone();
        let amount = if mutation == 0 {
            "US$9,999/oz"
        } else {
            "US$1,479/oz"
        };
        match mutation {
            0 => {
                let PublicValue::Number { displayed, .. } = &mut changed.claim.value else {
                    panic!("numeric workbook Claim")
                };
                *displayed = Ok("9999".into());
            }
            1 => changed.key.subject = "Site B".into(),
            2 => changed.key.valid_period.year = 2031,
            3 => changed.key.line.occurrence += 1,
            4 => changed.claim.state = KnowledgeState::Known,
            _ => changed.claim.anchors[0].locator = "QTD!Z9".into(),
        }
        assert_eq!(changed.claim.claim_id.to_string(), original_id);
        assert_eq!(changed.claim.check_id().is_ok(), mutation < 4);
        let text = format!(
            "{} Q1 {} 100% basis AISC per ounce sold is {amount}.",
            changed.key.subject, changed.key.valid_period.year
        );
        let cited = text.replace(amount, &format!("[{amount}](fmine:{original_id})"));
        for report in [
            lint_cited(&cited, std::slice::from_ref(&changed)),
            lint_discover(&text, std::slice::from_ref(&changed)),
        ] {
            assert_eq!(report.findings.len(), 1, "mutation {mutation}");
            let finding = &report.findings[0];
            assert_eq!(
                finding.verdict,
                Verdict::Unchecked,
                "mutation {mutation}: {}",
                report.to_json()
            );
            assert_eq!(finding.reason, "source_claim_identity_mismatch");
            assert_eq!(finding.claim_id.as_deref(), Some(original_id.as_str()));
            assert_eq!(
                finding.source.as_ref().map(KeyedCell::to_json),
                Some(changed.to_json())
            );
        }
    }
}

#[test]
fn invalid_competing_source_remains_visible_to_discovery() {
    let valid = source("Site A", "2030Q1", OwnershipBasis::Consolidated100);
    let mut tampered = source("Site A", "2030Q2", OwnershipBasis::Consolidated100);
    let PublicValue::Number { displayed, .. } = &mut tampered.claim.value else {
        panic!("numeric workbook Claim")
    };
    *displayed = Ok("9999".into());
    let report = lint_discover(
        "Site A Q1 2030 100% basis AISC per ounce sold is US$1,479/oz.",
        &[valid.clone(), tampered.clone()],
    );
    assert_eq!(report.findings.len(), 1);
    let finding = &report.findings[0];
    assert_eq!(finding.verdict, Verdict::Ambiguous);
    assert_eq!(finding.reason, "multiple_claims_matching_stated_keys");
    assert!(finding.source.is_none());
    assert_eq!(finding.candidates.len(), 2);
    for source in [valid, tampered] {
        assert!(
            finding
                .candidates
                .iter()
                .any(|c| c.to_json() == source.to_json())
        );
    }
}

#[test]
fn same_value_claims_with_competing_attributable_shares_cannot_be_interchanged() {
    let old = source(
        "Site A",
        "2030Q1",
        OwnershipBasis::Attributable {
            num: 3,
            den: 5,
            source: ShareSource::Printed,
        },
    );
    // Both supported canonical labels mean AISC in the bounded prose grammar; changing the source table
    // cannot make the unprinted exact share unambiguous.
    for line in [
        "summary:aisc_usd_per_oz",
        "all-in sustaining costs per ounce",
    ] {
        let newer = source_line(
            "Site A",
            "2030Q2",
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Printed,
            },
            line,
        );
        no_verified(
            &draft(&old, "attributable", "US$1,479/oz"),
            &[old.clone(), newer.clone()],
        );
        no_verified(
            &draft(&newer, "attributable", "US$1,479/oz"),
            &[old.clone(), newer.clone()],
        );
    }
}

#[test]
fn parentheses_outside_a_numeric_link_do_not_silently_erase_an_accounting_sign() {
    let claim = source("Site A", "2030Q1", OwnershipBasis::Consolidated100);
    let positive = draft(&claim, "100% basis", "US$1,479/oz");
    let negative = positive.replace("is [", "is ([").replace(").", ")).");
    no_verified(&negative, std::slice::from_ref(&claim));
    for sign in ["–", "—"] {
        no_verified(
            &positive.replace("is [", &format!("is {sign}[")),
            std::slice::from_ref(&claim),
        );
    }
}

#[test]
fn conflicting_metadata_unicode_and_malformed_links_never_verify() {
    let claim = source("Site A", "2030Q1", OwnershipBasis::Consolidated100);
    let other = source("Site B", "2030Q1", OwnershipBasis::Consolidated100);
    let valid = draft(&claim, "100% basis", "US$1,479/oz");
    assert_eq!(
        lint_cited(&valid, std::slice::from_ref(&claim)).findings[0].verdict,
        Verdict::Verified
    );
    let cases = [
        valid.replace("Site A", "Site A Site B"),
        valid.replace("Site A", "Site A\u{200b}"),
        valid.replace("Site A", "Site A\u{301}"),
        valid.replace("Site A", "Site AЖ"),
        valid.replace("AISC", "TCC AISC"),
        valid.replace("100% basis", "attributable 100% basis"),
        valid.replace("Q1 2030", "Q1 2030 Q2 2030"),
        valid.replace("per ounce sold", "per ounce produced per ounce sold"),
        valid.replace("US$1,479/oz", "US$1,479.00/oz"),
        valid.replace("US$1,479/oz", "US$1.479e3/oz"),
        valid.replace("[US$", "\\[US$"),
        valid.replace(").", ") extra."),
        valid.replace(").", " \"title\")."),
        valid.replace(
            &claim.claim.claim_id.to_string(),
            &other.claim.claim_id.to_string(),
        ),
    ];
    for text in cases {
        no_verified(&text, &[claim.clone(), other.clone()]);
    }
}
