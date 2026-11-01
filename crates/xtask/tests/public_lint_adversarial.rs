//! Adversarial known answers for the bounded cited-equality surface; all source assertions are invented.
use fmine_claim::{AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_public::key::{
    KeyedCell, LineKey, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue, Scope,
    ShareSource, Vintage,
};
use fmine_public::lint::{Verdict, lint_cited};

fn source(subject: &str, release: &str, basis: OwnershipBasis) -> KeyedCell {
    source_line(subject, release, basis, "summary:aisc_usd_per_oz")
}

fn source_line(subject: &str, release: &str, basis: OwnershipBasis, line: &str) -> KeyedCell {
    let key = PublicKey {
        issuer: "synthetic".into(),
        subject: subject.into(),
        valid_period: Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: release.into(),
            file_sha256: "ab".repeat(32),
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
    let locator = format!("{file}:QTD!A1");
    let claim = Claim::new(
        "aisc",
        PublicValue::Number {
            stored: "1479".into(),
            displayed: Ok("1,479".into()),
            under_status: None,
        },
        &key.canonical_tag(&locator),
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::IntensiveWeighted,
        EvidenceClass::ISSUER,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        ConfidentialityTier::Public,
        vec![],
        vec![],
        vec![SourceAnchor::new(&file, [0xab; 32], "QTD!A1")],
    );
    KeyedCell {
        key,
        claim,
        locator,
        printed_basis: basis,
        basis_evidence: None,
        status_evidence: None,
        label_as_printed: None,
        unit_evidence: None,
    }
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
