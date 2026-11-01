//! Known-answer and mutation checks for the bounded cited-equality contract, not general prose validation.

use fmine_core::hash::{Digest32, sha256};
use fmine_io::xlsx::{
    MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
};
use fmine_public::key::{
    KeyedCell, LineKey, NilReason, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue,
    Scope, ShareSource, Vintage, classify, public_claim,
};
use fmine_public::lint::{LintMode, Verdict, lint_cited, lint_discover};

#[allow(clippy::expect_used)] // A malformed invented fixture must fail its test immediately.
fn observation(value: &str, edit: impl FnOnce(&mut PublicKey)) -> KeyedCell {
    let mut key = PublicKey {
        issuer: "synthetic".into(),
        subject: "Site A".into(),
        valid_period: Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: "2030Q1".into(),
            file_sha256: String::new(),
        },
        ownership_basis: OwnershipBasis::Unprinted,
        scope: Scope::Site,
        unit: "usd_per_oz".into(),
        line: LineKey {
            label: "summary:aisc_usd_per_oz".into(),
            occurrence: 0,
        },
    };
    edit(&mut key);
    let file = format!("synthetic-lint-{}.xlsx", key.reporting_vintage.release);
    let format = value.split_once('.').map_or_else(
        || "0".into(),
        |(_, frac)| format!("0.{}", "0".repeat(frac.len())),
    );
    let bytes = build_minimal_xlsx_styled(
        &[MinimalSheet {
            name: "Data".into(),
            hidden: false,
            cells: vec![(
                "B1".into(),
                MinimalCell::Styled {
                    raw: value.into(),
                    xf: 0,
                },
            )],
        }],
        Some(&MinimalStyles {
            num_fmts: vec![(164, format)],
            xfs: vec![164],
        }),
    );
    let workbook = Workbook::from_bytes(&file, &bytes).expect("synthetic source");
    key.reporting_vintage.file_sha256 = Digest32(workbook.sha256).to_hex();
    let value = classify(workbook.sheets[0].cell(1, 2), None).expect("source value");
    public_claim(&workbook, "Data", "B1", key, value)
}

fn sentence(c: &KeyedCell, amount: &str) -> String {
    format!(
        "{} Q1 2030 ownership basis unprinted AISC per ounce sold is [{amount}](fmine:{}).",
        c.key.subject, c.claim.claim_id
    )
}

fn verdict(text: &str, claims: &[KeyedCell]) -> Verdict {
    let report = lint_cited(text, claims);
    assert_eq!(report.findings.len(), 1, "{}", report.to_json());
    report.findings[0].verdict
}

#[test]
fn printed_intervals_are_contained_exactly_without_inventing_precision() {
    let c = observation("1637", |_| {});
    for (amount, expected) in [
        ("$1,637/oz", Verdict::Verified),
        ("US$1,637/oz", Verdict::Verified),
        ("$1.637k/oz", Verdict::Verified),
        ("$1.6 thousand/oz", Verdict::Verified),
        ("$1,640/oz", Verdict::ValueMismatch),
        ("$1.63k/oz", Verdict::ValueMismatch),
        ("$1,637.0/oz", Verdict::RoundingBeyondTolerance),
        ("$1,637.2/oz", Verdict::RoundingBeyondTolerance),
    ] {
        assert_eq!(
            verdict(&sentence(&c, amount), std::slice::from_ref(&c)),
            expected,
            "{amount}"
        );
    }
    let huge = observation("9007199254740992", |_| {});
    assert_eq!(
        verdict(
            &sentence(&huge, "$9007199254740990/oz"),
            std::slice::from_ref(&huge)
        ),
        Verdict::Verified,
        "the source format renders fifteen significant digits: {:?}",
        huge.claim.value
    );
    assert_ne!(
        verdict(
            &sentence(&huge, "$9007199254740992/oz"),
            std::slice::from_ref(&huge)
        ),
        Verdict::Verified
    );
    let negative = observation("-708", |_| {});
    assert_eq!(
        verdict(
            &sentence(&negative, "($708/oz)"),
            std::slice::from_ref(&negative)
        ),
        Verdict::Verified
    );
}

#[test]
fn equal_value_links_cannot_swap_subject_metric_period_ownership_or_units() {
    let a = observation("1637", |_| {});
    let b = observation("1637", |k| k.subject = "Site B".into());
    let cost = observation("1637", |k| k.line.label = "summary:cos_usd_per_oz".into());
    let claims = [a.clone(), b.clone(), cost.clone()];
    let valid = sentence(&a, "$1,637/oz");
    assert_eq!(
        verdict(
            &valid.replace(&a.claim.claim_id.to_string(), &b.claim.claim_id.to_string()),
            &claims
        ),
        Verdict::BasisMismatch
    );
    assert_eq!(
        verdict(
            &valid.replace(
                &a.claim.claim_id.to_string(),
                &cost.claim.claim_id.to_string()
            ),
            &claims
        ),
        Verdict::BasisMismatch
    );
    for (from, to, expected) in [
        ("Q1 2030", "Q2 2030", Verdict::PeriodMismatch),
        (
            "ownership basis unprinted",
            "100% basis",
            Verdict::BasisMismatch,
        ),
        (
            "per ounce sold",
            "per ounce produced",
            Verdict::BasisMismatch,
        ),
        ("$1,637/oz", "$1,637/lb", Verdict::BasisMismatch),
        ("$1,637/oz", "C$1,637/oz", Verdict::Unchecked),
        ("$1,637/oz", "C$1,637 USD/oz", Verdict::BasisMismatch),
        ("ownership basis unprinted ", "", Verdict::Ambiguous),
        ("per ounce sold ", "", Verdict::Ambiguous),
        ("AISC", "scorecard AISC", Verdict::Unchecked),
        ("AISC", "API scorecard AISC", Verdict::BasisMismatch),
    ] {
        assert_eq!(
            verdict(&valid.replace(from, to), &claims),
            expected,
            "{from} -> {to}"
        );
    }
    let cos = sentence(&cost, "$1,637/oz").replace("AISC", "cost of sales");
    assert_eq!(verdict(&cos, &claims), Verdict::Verified);
    assert_eq!(
        verdict(
            &cos.replace("cost of sales", "by-product cost of sales"),
            &claims
        ),
        Verdict::BasisMismatch
    );
}

#[test]
fn distinct_credit_and_production_bases_have_positive_known_answers() {
    let co = observation("1100", |k| {
        k.line.label = "total cash costs per ounce (on a co-product basis)".into()
    });
    let draft = sentence(&co, "$1,100/oz").replace("AISC", "co-product TCC");
    assert_eq!(
        verdict(&draft, std::slice::from_ref(&co)),
        Verdict::Verified
    );
    let discovered = draft.replace(
        &format!("[$1,100/oz](fmine:{})", co.claim.claim_id),
        "$1,100/oz",
    );
    assert_eq!(
        lint_discover(&discovered, std::slice::from_ref(&co)).findings[0].verdict,
        Verdict::Verified
    );
    assert_eq!(
        verdict(
            &draft.replace("co-product", "by-product"),
            std::slice::from_ref(&co)
        ),
        Verdict::BasisMismatch
    );
    assert_eq!(
        verdict(&draft.replace("co-product ", ""), &[co]),
        Verdict::Ambiguous
    );
    for (basis, word) in [
        (OwnershipBasis::Consolidated100, "100% basis"),
        (
            OwnershipBasis::Attributable {
                num: 21,
                den: 25,
                source: ShareSource::Printed,
            },
            "attributable",
        ),
    ] {
        let c = observation("35", |k| {
            k.unit = "koz".into();
            k.ownership_basis = basis;
            k.line.label = if basis == OwnershipBasis::Consolidated100 {
                "summary:au_prod_100_koz"
            } else {
                "summary:au_prod_attr_koz"
            }
            .into();
        });
        let draft = format!(
            "Site A Q1 2030 {word} gold production is [35 koz](fmine:{}).",
            c.claim.claim_id
        );
        assert_eq!(verdict(&draft, std::slice::from_ref(&c)), Verdict::Verified);
        let discovered = draft.replace(&format!("[35 koz](fmine:{})", c.claim.claim_id), "35 koz");
        assert_eq!(
            lint_discover(&discovered, std::slice::from_ref(&c)).findings[0].verdict,
            Verdict::Verified
        );
        assert_eq!(
            verdict(&draft.replace("35 koz", "0.035 Moz"), &[c]),
            Verdict::BasisMismatch,
            "no implicit unit conversion"
        );
    }
}

#[test]
fn negation_comparisons_ranges_extra_links_and_probabilities_do_not_verify() {
    let c = observation("1637", |_| {});
    let valid = sentence(&c, "$1,637/oz");
    for draft in [
        valid.replace(" is ", " is not "),
        valid.replace(" is ", " is above "),
        valid.replace(" is ", " is probably "),
        format!("If production rises, {valid}"),
        valid.replace(".", "?"),
        valid.replace("$1,637/oz", "$1,637 to 1,640/oz"),
        valid.replace("AISC", "AISC SECRET_DRAFT_TEXT"),
        valid.replace(").", &format!(")[not](fmine:{}).", c.claim.claim_id)),
    ] {
        let report = lint_cited(&draft, std::slice::from_ref(&c));
        assert!(!report.findings.is_empty(), "{draft}");
        assert!(
            report
                .findings
                .iter()
                .all(|f| f.verdict != Verdict::Verified),
            "{draft}"
        );
        assert!(!report.unchecked_spans.is_empty());
        assert!(!report.to_json().contains("SECRET_DRAFT_TEXT"));
        assert!(!report.to_markdown().contains("SECRET_DRAFT_TEXT"));
    }
    assert_eq!(
        verdict(
            &valid.replace("AISC", "probability of AISC"),
            std::slice::from_ref(&c)
        ),
        Verdict::UncalibratedProbability
    );
    let unlinked = valid.replace(
        &format!("[$1,637/oz](fmine:{})", c.claim.claim_id),
        "$1,637/oz",
    );
    assert_eq!(
        verdict(&unlinked, std::slice::from_ref(&c)),
        Verdict::Uncited
    );
    assert_eq!(
        verdict(
            &valid.replace(
                &c.claim.claim_id.to_string(),
                "clm_00000000000000000000000000000000"
            ),
            &[c]
        ),
        Verdict::Ambiguous
    );
    let prose = lint_cited(
        "A qualitative statement remains outside the supported predicates.",
        &[],
    );
    assert!(prose.findings.is_empty());
    assert_eq!(prose.unchecked_spans.len(), 1);
}

#[test]
fn snapshot_latest_and_explicit_staleness_preserve_old_assertions() {
    let first = observation("1479", |_| {});
    let later = observation("1474", |k| k.reporting_vintage.release = "2031Q1".into());
    let draft = sentence(&first, "$1,479/oz");
    assert_eq!(
        verdict(&draft, std::slice::from_ref(&first)),
        Verdict::Verified
    );
    let mut claims = vec![first.clone(), later.clone()];
    let report = lint_cited(&draft, &claims);
    assert_eq!(report.findings[0].verdict, Verdict::Stale);
    assert_eq!(
        report.findings[0].later[0].claim.claim_id,
        later.claim.claim_id
    );
    assert_eq!(
        verdict(&sentence(&later, "$1,474/oz"), &claims),
        Verdict::Verified
    );
    claims.reverse();
    assert_eq!(lint_cited(&draft, &claims).to_json(), report.to_json());
    let mut explicit = first.clone();
    explicit.claim.stale = true;
    assert_eq!(verdict(&draft, &[explicit]), Verdict::Stale);
    assert_eq!(
        verdict(&draft, &[first.clone(), first.clone()]),
        Verdict::Ambiguous
    );
    let concurrent = observation("1480", |_| {});
    assert_eq!(verdict(&draft, &[first, concurrent]), Verdict::Ambiguous);
}

#[test]
fn token_display_and_printed_basis_limits_never_become_verified_numbers() {
    let original = observation("0", |_| {});
    let draft = sentence(&original, "$0/oz");
    for value in [
        PublicValue::Missing,
        PublicValue::NotApplicable {
            token: "n/a".into(),
        },
        PublicValue::Nil {
            token: "0".into(),
            reason: NilReason::Withheld,
        },
        PublicValue::Number {
            stored: "1.2".into(),
            displayed: Err("general_non_integer"),
            under_status: None,
        },
    ] {
        let mut c = original.clone();
        c.claim.value = value;
        assert_eq!(verdict(&draft, &[c]), Verdict::Unchecked);
    }
    let mut c = observation("1637", |k| {
        k.ownership_basis = OwnershipBasis::Attributable {
            num: 49,
            den: 200,
            source: ShareSource::Registry,
        }
    });
    c.printed_basis = OwnershipBasis::Attributable {
        num: 19,
        den: 40,
        source: ShareSource::Printed,
    };
    c.basis_evidence = Some("SYNTHETIC-CORRECTION".into());
    let draft = sentence(&c, "$1,637/oz").replace("ownership basis unprinted", "attributable");
    assert_eq!(verdict(&draft, &[c]), Verdict::Ambiguous);
}

#[test]
fn report_binds_inputs_findings_and_exact_spans_without_copying_prose() {
    let c = observation("1637", |_| {});
    let draft = format!(
        "Private context remains unchecked.\n{}",
        sentence(&c, "$1,637/oz")
    );
    let report = lint_cited(&draft, std::slice::from_ref(&c));
    assert_eq!(report.draft_digest, sha256(draft.as_bytes()));
    let f = &report.findings[0];
    assert_eq!(&draft[f.span.0..f.span.1], f.raw);
    assert_eq!(f.source.as_ref().expect("canonical source"), &c);
    let json = report.to_json();
    assert!(json.contains("\"numbers\":\"1\""));
    assert!(json.contains("\"draft_sha256\""));
    assert!(json.contains(&c.key.reporting_vintage.file_sha256));
    assert!(!json.contains("Private context"));
    let mut mutated = report.clone();
    mutated.findings[0].verdict = Verdict::ValueMismatch;
    assert_ne!(mutated.digest(), report.digest());
    assert_ne!(
        lint_cited(&(draft.clone() + "\n"), &[c]).digest(),
        report.digest()
    );
}

fn unlinked(c: &KeyedCell, amount: &str) -> String {
    sentence(c, amount).replace(&format!("[{amount}](fmine:{})", c.claim.claim_id), amount)
}

#[test]
fn discovery_unique_claim_runs_the_same_exact_checks_and_preserves_spans() {
    let c = observation("1637", |_| {});
    for (amount, expected) in [
        ("$1,637/oz", Verdict::Verified),
        ("US$1,637/oz", Verdict::Verified),
        ("$1.6 thousand/oz", Verdict::Verified),
        ("$1,640/oz", Verdict::ValueMismatch),
        ("$1,637.0/oz", Verdict::RoundingBeyondTolerance),
    ] {
        let draft = unlinked(&c, amount);
        let report = lint_discover(&draft, std::slice::from_ref(&c));
        assert_eq!(report.mode, LintMode::Discovered);
        assert_eq!(report.findings.len(), 1);
        let f = &report.findings[0];
        assert_eq!((f.mode, f.verdict), (LintMode::Discovered, expected));
        assert_eq!(f.source.as_ref(), Some(&c));
        assert_eq!(
            f.claim_id.as_deref(),
            Some(c.claim.claim_id.to_string().as_str())
        );
        assert_eq!(&draft[f.span.0..f.span.1], f.raw);
        assert!(report.to_json().contains("\"mode\":\"discovered\""));
        let markdown = report.to_markdown();
        assert!(markdown.contains("| discovered |"));
        assert!(markdown.contains(&c.claim.claim_id.to_string()));
        assert!(markdown.contains(&c.locator));
        assert!(
            report
                .to_json()
                .contains("General-prose discovery precision has not been measured")
        );
        assert_eq!(
            lint_cited(&draft, std::slice::from_ref(&c)).findings[0].verdict,
            Verdict::Uncited
        );
    }
}

#[test]
fn discovery_never_breaks_a_semantic_tie_by_value_vintage_or_source_order() {
    let first = observation("1637", |_| {});
    let wrong_value = observation("900", |_| {});
    let later = observation("1638", |k| k.reporting_vintage.release = "2031Q1".into());
    let draft = unlinked(&first, "$1,637/oz");
    for competitor in [wrong_value, later, first.clone()] {
        let mut claims = vec![first.clone(), competitor];
        let report = lint_discover(&draft, &claims);
        let f = &report.findings[0];
        assert_eq!(f.verdict, Verdict::Ambiguous);
        assert_eq!(f.candidates.len(), 2);
        assert!(f.source.is_none());
        assert!(f.claim_id.is_none());
        let markdown = report.to_markdown();
        for candidate in &f.candidates {
            assert!(markdown.contains(&candidate.claim.claim_id.to_string()));
            assert!(markdown.contains(&candidate.locator));
        }
        claims.reverse();
        assert_eq!(report.to_json(), lint_discover(&draft, &claims).to_json());
    }
    let mut stale = first.clone();
    stale.claim.stale = true;
    assert_eq!(
        lint_discover(&draft, &[stale]).findings[0].verdict,
        Verdict::Stale
    );
    let mut missing = first;
    missing.claim.value = PublicValue::Missing;
    assert_eq!(
        lint_discover(&draft, &[missing]).findings[0].verdict,
        Verdict::Unchecked
    );
}

#[test]
fn discovery_requires_every_semantic_key_and_never_repairs_explicit_citations() {
    let a = observation("1637", |_| {});
    let b = observation("1637", |k| k.subject = "Site B".into());
    let claims = [a.clone(), b.clone()];
    let valid = unlinked(&a, "$1,637/oz");
    for (from, to, expected) in [
        ("Site A", "Site A Site B", Verdict::Ambiguous),
        ("Site A", "Unregistered Mine", Verdict::Unchecked),
        ("AISC", "AISC TCC", Verdict::Unchecked),
        ("AISC", "recovery", Verdict::Unchecked),
        ("AISC", "cost of sales", Verdict::Unchecked),
        ("AISC", "sales", Verdict::Unchecked),
        ("Q1 2030", "Q2 2030", Verdict::Uncited),
        ("Q1 2030", "", Verdict::Ambiguous),
        ("ownership basis unprinted", "", Verdict::Ambiguous),
        ("ownership basis unprinted", "100% basis", Verdict::Uncited),
        ("per ounce sold", "", Verdict::Ambiguous),
        ("per ounce sold", "per ounce produced", Verdict::Uncited),
        ("$1,637/oz", "$1,637/lb", Verdict::Uncited),
        (" is ", " is not ", Verdict::Unchecked),
    ] {
        let report = lint_discover(&valid.replace(from, to), &claims);
        assert_eq!(report.findings.len(), 1, "{from} -> {to}");
        assert_eq!(report.findings[0].verdict, expected, "{from} -> {to}");
    }
    let wrong_link = sentence(&a, "$1,637/oz")
        .replace(&a.claim.claim_id.to_string(), &b.claim.claim_id.to_string());
    let report = lint_discover(&wrong_link, &claims);
    assert_eq!(report.findings[0].mode, LintMode::Cited);
    assert_eq!(report.findings[0].verdict, Verdict::BasisMismatch);
    let malformed = sentence(&a, "$1,637/oz").replace(&a.claim.claim_id.to_string(), "not-a-claim");
    assert_ne!(
        lint_discover(&malformed, &claims).findings[0].verdict,
        Verdict::Verified
    );
    let private = format!("PRIVATE_DRAFT_PROSE.\n{valid}");
    let report = lint_discover(&private, &claims);
    assert_eq!(report.findings[0].verdict, Verdict::Verified);
    assert!(!report.to_json().contains("PRIVATE_DRAFT_PROSE"));
    assert!(!report.to_markdown().contains("PRIVATE_DRAFT_PROSE"));
    assert_eq!(report.unchecked_spans.len(), 1);
}

#[test]
#[ignore = "needs the registered Barrick workbooks"]
fn real_mine_stats_equalities_and_e474_staleness_use_canonical_source_claims() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = fmine_public::load_issuer_profile(&root, "barrick").expect("profile");
    let inventory =
        fmine_public::key::mine_stats::load_claims(&profile, &root).expect("verified claims");
    assert!(inventory.refusals.is_empty());
    let at = |suffix: &str| {
        inventory
            .claims
            .iter()
            .find(|c| c.locator == suffix)
            .expect("cited source")
    };
    let mut draft = String::new();
    for (cell, basis, metric, amount) in [
        ("G19", "attributable", "AISC per ounce sold", "$2,577/oz"),
        (
            "F19",
            "attributable",
            "by-product TCC per ounce sold",
            "$2,164/oz",
        ),
        (
            "E19",
            "attributable",
            "cost of sales per ounce sold",
            "$2,736/oz",
        ),
        ("B19", "100% basis", "gold production", "41 koz"),
        ("C19", "attributable", "gold production", "35 koz"),
        ("D19", "attributable", "gold sales", "37 koz"),
    ] {
        let c = at(&format!("Barrick_Q2_2026_Mine_Stats.xlsx:QTD!{cell}"));
        draft.push_str(&format!(
            "North Mara Q2 2026 {basis} {metric} is [{amount}](fmine:{}).\n",
            c.claim.claim_id
        ));
    }
    let report = lint_cited(&draft, &inventory.claims);
    assert_eq!(report.findings.len(), 6);
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.verdict == Verdict::Verified),
        "{}",
        report.to_json()
    );
    let first = at("Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23");
    let e474 = format!(
        "Total Gold Q1 2024 ownership basis unprinted AISC per ounce sold is [$1,479/oz](fmine:{}).",
        first.claim.claim_id
    );
    let full = lint_cited(&e474, &inventory.claims);
    assert_eq!(full.findings[0].verdict, Verdict::Stale);
    assert!(
        full.findings[0]
            .later
            .iter()
            .any(|c| c.locator == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
    );
    let earlier: Vec<_> = inventory
        .claims
        .iter()
        .filter(|c| c.key.reporting_vintage.release.as_str() <= "2024Q1")
        .cloned()
        .collect();
    assert_eq!(verdict(&e474, &earlier), Verdict::Verified);
}
