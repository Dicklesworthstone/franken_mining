//! Independent source-selection controls for BD-341's descriptive public guidance track record.
#![cfg(test)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_calib::guidance_scores::Side;
use fmine_claim::{AdditivityClass, Claim};
use fmine_core::hash::sha256;
use fmine_evidence::RefusalCode;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};
use fmine_numerics::exact_feas::Rat;
use fmine_public::guidance::track_record::{self, TrackRecordReport};
use fmine_public::key::{
    KeyedCell, OwnershipBasis, PublicValue, Scope, StatusKind, SubjectStatus, canonical_value_tag,
};
use fmine_public::sources::{DocumentRow, IssuerProfile, RegisteredSource, SourceKind};
use fmine_public::transcribe::TranscriptionReport;
use fmine_registry::transcriptions::{Agreement, parse_token};

const CUT: &str = "2032-02-01";

fn fixture() -> (IssuerProfile, TranscriptionReport, Vec<KeyedCell>) {
    let report = track_record::synthetic().expect("invented evidence through the real adapter");
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.retrieved_utc = format!("{CUT}T00:00:00Z");
    let mut documents = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut cells = BTreeMap::new();
    let mut accepted = Vec::new();
    for row in &report.rows {
        documents.insert(row.document.file.clone(), row.document.clone());
        accepted.push(row.guidance.clone());
        for c in row
            .first_published
            .sources
            .iter()
            .chain(&row.latest_registered.sources)
        {
            let name = c.locator.split_once(':').unwrap().0.to_string();
            sources.insert(
                name.clone(),
                RegisteredSource {
                    file_name: name,
                    kind: SourceKind::MineStats,
                    quarter: c.key.reporting_vintage.release.clone(),
                    url: "https://example.invalid/synthetic.xlsx".into(),
                    url_basis: "synthetic".into(),
                    bytes: 1,
                    sha256_hex: c.key.reporting_vintage.file_sha256.clone(),
                },
            );
            cells.insert(c.locator.clone(), c.clone());
        }
    }
    profile.sources = sources.into_values().collect();
    profile.documents = documents
        .values()
        .map(|d| DocumentRow {
            file: d.file.clone(),
            dir: "invented".into(),
            kind: d.kind.clone(),
            url: "https://example.invalid/synthetic.pdf".into(),
            url_basis: "synthetic".into(),
            published_at: d.published_at.clone(),
            published_at_basis: d.published_at_basis.clone(),
            pages: 1,
            bytes: 1,
            sha256: d.sha256.clone(),
        })
        .collect();
    let transcriptions = TranscriptionReport {
        registry: "synthetic:independently-reassembled-test-inputs".into(),
        documents: documents.into_values().collect(),
        entries: accepted.len() * 2,
        agreement: Agreement {
            accepted,
            ..Agreement::default()
        },
        tie: None,
    };
    (profile, transcriptions, cells.into_values().collect())
}

fn run(p: &IssuerProfile, t: &TranscriptionReport, c: &[KeyedCell]) -> TrackRecordReport {
    track_record::assess(p, t, c, None, CUT).expect("evidence selection")
}

#[test]
fn original_and_restatement_have_separate_exact_scores_and_keep_missing_member() {
    let r = track_record::synthetic().unwrap();
    assert_eq!(r.rows.len(), 2);
    let row = &r.rows[0];
    assert_eq!(row.site, "example-mine");
    let first = row.first_published.score.unwrap();
    assert_eq!(first.z, Some(Rat::new(3, 2).unwrap()));
    assert_eq!(first.side, Side::Above);
    assert_eq!(
        row.latest_registered.score.unwrap().z,
        Some(Rat::new(1, 2).unwrap())
    );
    assert_eq!(row.latest_registered.status, "later_comparative");
    assert_eq!(r.rows[1].site, "missing-mine");
    assert!(r.rows[1].first_published.score.is_none());
    let first = &r
        .summaries
        .iter()
        .find(|s| s.selector == "first_published")
        .unwrap()
        .scores;
    let latest = &r
        .summaries
        .iter()
        .find(|s| s.selector == "latest_registered")
        .unwrap()
        .scores;
    assert_eq!(
        (
            first.n_members,
            first.n_resolved,
            first.n_unresolved,
            first.inside
        ),
        (2, 1, 1, 0)
    );
    assert_eq!(
        (
            latest.n_members,
            latest.n_resolved,
            latest.n_unresolved,
            latest.inside
        ),
        (2, 1, 1, 1)
    );
    assert_eq!(
        first.whole_cohort_hit_bounds.unwrap().lo,
        Rat::from_integer(0)
    );
    assert_eq!(
        first.whole_cohort_hit_bounds.unwrap().hi,
        Rat::new(1, 2).unwrap()
    );
    assert_eq!(
        latest.whole_cohort_hit_bounds.unwrap().lo,
        Rat::new(1, 2).unwrap()
    );
    assert_eq!(
        latest.whole_cohort_hit_bounds.unwrap().hi,
        Rat::from_integer(1)
    );
    assert_eq!(r.to_json(), track_record::synthetic().unwrap().to_json());
    assert!(r.to_json().contains("\"lane\":\"synthetic\""));
    assert!(
        r.to_json()
            .contains("Effective sample size is not estimated")
    );
    assert!(
        r.to_json()
            .contains("no retrospective forecast registration")
    );
    let md = r.to_markdown();
    assert!(md.find("| missing-mine |").unwrap() < md.find("## Sources").unwrap());
    assert!(md.contains("125; above; z 3/2"));
}

#[test]
fn later_comparative_never_fills_missing_original_release() {
    let (p, t, mut c) = fixture();
    c.retain(|c| c.key.reporting_vintage.release != "2030Q4");
    let r = run(&p, &t, &c);
    assert!(r.rows[0].first_published.score.is_none());
    assert_eq!(
        r.rows[0].latest_registered.status,
        "comparative_only_original_missing"
    );
    assert!(r.rows[0].latest_registered.score.is_some());
    assert_eq!(
        (
            r.summaries[0].scores.n_members,
            r.summaries[0].scores.n_resolved
        ),
        (2, 0)
    );
}

#[test]
fn stale_original_and_wrong_basis_stay_unresolved_without_changing_denominator() {
    let (p, t, c) = fixture();
    let mut stale = c.clone();
    stale[0].claim.stale = true;
    let r = run(&p, &t, &stale);
    assert_eq!(
        r.rows[0].first_published.refusal.as_ref().unwrap().code,
        RefusalCode::StaleAnchor502.as_str()
    );
    assert!(r.rows[0].latest_registered.score.is_some());
    assert_eq!(r.summaries[0].scores.n_members, 2);

    let mut wrong = c;
    wrong[0].key.ownership_basis = OwnershipBasis::Consolidated100;
    refresh_claim(&mut wrong[0]);
    let r = run(&p, &t, &wrong);
    assert_eq!(
        r.rows[0].first_published.refusal.as_ref().unwrap().code,
        RefusalCode::BasisMismatch105.as_str()
    );
    assert_eq!(r.rows[0].first_published.sources.len(), 1);
    assert_eq!(r.summaries[0].scores.n_members, 2);
}

fn refresh_claim(c: &mut KeyedCell) {
    let old = &c.claim;
    c.claim = Claim::new(
        &c.key.line.label,
        old.value.clone(),
        &canonical_value_tag(&c.key, &old.value),
        old.representation.clone(),
        old.support,
        old.additivity,
        old.evidence,
        old.epistemic,
        old.state,
        old.confidentiality.clone(),
        old.uu_terms.clone(),
        old.parents.clone(),
        old.anchors.clone(),
    );
}

fn add_guidance(
    p: &mut IssuerProfile,
    t: &mut TranscriptionReport,
    file: &str,
    date: &str,
    token: &str,
    basis: &str,
) {
    let mut document = p.documents[0].clone();
    document.file = file.into();
    document.published_at = date.into();
    document.sha256 = sha256(format!("invented document {file}|{token}").as_bytes()).to_hex();
    p.documents.push(document.clone());
    let mut checked = t.documents[0].clone();
    checked.file = file.into();
    checked.published_at = date.into();
    checked.sha256 = document.sha256.clone();
    t.documents.push(checked);
    let mut a = t
        .agreement
        .accepted
        .iter()
        .find(|a| a.quantity.contains("example_mine"))
        .unwrap()
        .clone();
    a.locator.doc = file.into();
    a.token = token.into();
    a.value = parse_token(token).unwrap();
    a.doc_sha256 = document.sha256;
    a.basis = basis.into();
    t.agreement.accepted.push(a);
}

#[test]
fn revisions_are_visible_but_cannot_cherry_pick_the_default_score_or_duplicate_members() {
    let (mut p, mut t, c) = fixture();
    add_guidance(
        &mut p,
        &mut t,
        "revised.txt",
        "2030-10-01",
        "120 - 130",
        "attributable:8/10",
    );
    let r = run(&p, &t, &c);
    let revisions: Vec<_> = r.rows.iter().filter(|r| r.site == "example-mine").collect();
    assert_eq!(revisions.len(), 2);
    assert_eq!(
        revisions.iter().filter(|r| r.earliest_registered).count(),
        1
    );
    assert_eq!(
        revisions
            .iter()
            .find(|r| !r.earliest_registered)
            .unwrap()
            .first_published
            .score
            .unwrap()
            .side,
        Side::Inside
    );
    assert_eq!(
        (
            r.summaries[0].scores.n_members,
            r.summaries[0].scores.inside
        ),
        (2, 0)
    );
    t.agreement.accepted.reverse();
    p.documents.reverse();
    t.documents.reverse();
    let mut reversed = c;
    reversed.reverse();
    assert_eq!(r.to_json(), run(&p, &t, &reversed).to_json());
}

#[test]
fn same_date_guidance_and_duplicate_outcomes_are_explicit_ambiguities() {
    let (mut p, mut t, mut c) = fixture();
    add_guidance(
        &mut p,
        &mut t,
        "same-date.txt",
        "2030-01-15",
        "120 - 130",
        "attributable:4/5",
    );
    let r = run(&p, &t, &c);
    assert_eq!(r.rows.iter().filter(|r| r.ambiguous_earliest).count(), 2);
    assert_eq!(
        (
            r.summaries[0].scores.n_members,
            r.summaries[0].scores.n_resolved
        ),
        (2, 0)
    );
    c.push(c[0].clone());
    let r = run(&p, &t, &c);
    assert_eq!(
        r.rows
            .iter()
            .find(|r| r.site == "example-mine")
            .unwrap()
            .first_published
            .status,
        "ambiguous_outcome"
    );
}

#[test]
fn altered_projection_unregistered_digest_and_pending_reading_cannot_score() {
    let (p, mut t, c) = fixture();
    let mut altered = c.clone();
    altered[0].key.unit = "kt".into();
    let r = run(&p, &t, &altered);
    assert_eq!(
        r.rows[0].first_published.refusal.as_ref().unwrap().code,
        RefusalCode::FrozenRecordMutation507.as_str()
    );
    for value in [
        PublicValue::Number {
            stored: "125".into(),
            displayed: Ok("110".into()),
            under_status: None,
        },
        PublicValue::Number {
            stored: "125".into(),
            displayed: Ok("125".into()),
            under_status: Some(StatusKind::Withheld),
        },
    ] {
        let mut altered = c.clone();
        altered[0].claim.value = value;
        assert_eq!(
            run(&p, &t, &altered).rows[0]
                .first_published
                .refusal
                .as_ref()
                .unwrap()
                .code,
            RefusalCode::FrozenRecordMutation507.as_str(),
        );
    }
    let mut altered = c.clone();
    altered[0].locator = altered[0].locator.replace("C4", "C5");
    assert_eq!(
        run(&p, &t, &altered).rows[0]
            .first_published
            .refusal
            .as_ref()
            .unwrap()
            .code,
        RefusalCode::DigestMismatch505.as_str(),
    );
    let mut altered = p.clone();
    altered.sources[0].sha256_hex = sha256(b"different source").to_hex();
    assert_eq!(
        run(&altered, &t, &c).rows[0]
            .first_published
            .refusal
            .as_ref()
            .unwrap()
            .code,
        RefusalCode::DigestMismatch505.as_str()
    );
    t.agreement.single_reader.push((
        t.agreement.accepted[0].locator.clone(),
        "pending-role".into(),
    ));
    assert_eq!(
        track_record::assess(&p, &t, &c, None, CUT)
            .unwrap_err()
            .code,
        RefusalCode::NotObservable202.as_str()
    );
}

#[test]
fn late_guidance_and_future_outcomes_cannot_become_historical_successes() {
    let (mut p, mut t, c) = fixture();
    p.documents[0].published_at = "2031-02-01".into();
    t.documents[0].published_at = "2031-02-01".into();
    let r = run(&p, &t, &c);
    assert!(r.rows.iter().all(|r| r.first_published.score.is_none()));
    assert_eq!(r.rows[0].first_published.status, "guidance_timing_unproven");
    let (mut p, t, c) = fixture();
    p.retrieved_utc = "2030-06-30T00:00:00Z".into();
    let r = track_record::assess(&p, &t, &c, None, "2030-07-01").unwrap();
    assert!(r.rows.iter().all(|r| r.latest_registered.score.is_none()));
    let mut regional = c;
    regional[0].key.scope = Scope::Company;
    refresh_claim(&mut regional[0]);
    assert!(
        run(&p, &t, &regional).rows[0]
            .first_published
            .score
            .is_none()
    );
}

#[test]
fn production_and_cost_metrics_require_their_own_units_and_sold_definitions() {
    for (metric, unit, line, definition) in [
        ("production_kt", "kt", "cu_prod_attr_kt", ""),
        (
            "aisc_usd_per_oz",
            "usd_per_oz",
            "aisc_usd_per_oz",
            ";aisc=issuer-reported-per-sold-oz",
        ),
        (
            "aisc_usd_per_lb",
            "usd_per_lb",
            "aisc_usd_per_lb",
            ";aisc=issuer-reported-per-sold-lb",
        ),
        (
            "tcc_usd_per_oz",
            "usd_per_oz",
            "tcc_usd_per_oz",
            ";tcc=issuer-reported-per-sold-oz",
        ),
        (
            "c1_usd_per_lb",
            "usd_per_lb",
            "c1_usd_per_lb",
            ";c1=issuer-reported-per-sold-lb",
        ),
    ] {
        let (p, mut t, mut c) = fixture();
        for a in &mut t.agreement.accepted {
            a.quantity = a.quantity.replace("production_koz", metric);
            a.unit = unit.into();
            a.basis.push_str(definition);
        }
        for cell in &mut c {
            cell.key.unit = unit.into();
            cell.key.line.label = format!("summary:{line}");
            if !definition.is_empty() {
                cell.claim.additivity = AdditivityClass::IntensiveWeighted;
            }
            refresh_claim(cell);
        }
        let r = run(&p, &t, &c);
        assert_eq!(
            r.rows[0].first_published.score.unwrap().z,
            Some(Rat::new(3, 2).unwrap())
        );
        assert_eq!(r.summaries[0].scores.n_members, 2);
        if !definition.is_empty() {
            for a in &mut t.agreement.accepted {
                a.basis = a.basis.replace("per-sold", "per-produced");
            }
            let r = run(&p, &t, &c);
            assert_eq!(r.rows[0].first_published.status, "basis_unresolved");
            assert_eq!(r.summaries[0].scores.n_unresolved, 2);
        }
    }
}

#[test]
fn negative_production_is_unresolved_while_cost_credits_remain_scorable() {
    for (guidance, actual, cost) in [
        ("-10 - 120", "125", false),
        ("100 - 120", "-1", false),
        ("-20 - 0", "-10", true),
    ] {
        let (p, mut t, mut c) = fixture();
        for row in &mut t.agreement.accepted {
            row.token = guidance.into();
            row.value = parse_token(guidance).unwrap();
            if cost {
                row.quantity = row.quantity.replace("production_koz", "aisc_usd_per_oz");
                row.unit = "usd_per_oz".into();
                row.basis.push_str(";aisc=issuer-reported-per-sold-oz");
            }
        }
        c[0].claim.value = PublicValue::Number {
            stored: actual.into(),
            displayed: Ok(actual.into()),
            under_status: None,
        };
        for cell in &mut c {
            if cost {
                cell.key.unit = "usd_per_oz".into();
                cell.key.line.label = "summary:aisc_usd_per_oz".into();
                cell.claim.additivity = AdditivityClass::IntensiveWeighted;
            }
            refresh_claim(cell);
        }
        let r = run(&p, &t, &c);
        let first = &r.rows[0].first_published;
        assert_eq!(r.summaries[0].scores.n_members, 2);
        assert_eq!(first.sources.len(), 1);
        if cost {
            assert_eq!(first.score.unwrap().side, Side::Inside);
            assert_eq!(first.score.unwrap().z, Some(Rat::from_integer(0)));
            assert_eq!(r.summaries[0].scores.n_unresolved, 1);
        } else {
            assert!(first.score.is_none());
            assert_eq!(
                first.refusal.as_ref().unwrap().code,
                RefusalCode::OutsidePhysicalDomain109.as_str()
            );
            assert_eq!(r.summaries[0].scores.n_unresolved, 2);
        }
    }
}

#[test]
fn printed_zaldivar_accent_matches_ascii_transcription_and_site_queries() {
    let (p, mut t, mut c) = fixture();
    for row in &mut t.agreement.accepted {
        if row.quantity.contains("example_mine") {
            row.quantity = row.quantity.replace("example_mine", "zaldivar");
            row.locator.row = "Zaldívar (80%)".into();
        }
    }
    for cell in &mut c {
        cell.key.subject = "Zaldívar".into();
        refresh_claim(cell);
    }
    let ascii = track_record::assess(&p, &t, &c, Some("ZALDIVAR"), CUT).unwrap();
    let accented = track_record::assess(&p, &t, &c, Some("ZALDÍVAR"), CUT).unwrap();
    assert_eq!(ascii.to_json(), accented.to_json());
    assert_eq!(ascii.rows.len(), 1);
    assert_eq!(ascii.rows[0].site, "zaldivar");
    assert_eq!(
        ascii.rows[0].first_published.score.unwrap().z,
        Some(Rat::new(3, 2).unwrap())
    );
}

#[test]
fn annual_stub_disposals_and_partial_withholding_keep_members_unresolved() {
    let (mut p, t, c) = fixture();
    for status in [
        SubjectStatus::from_date(
            "Example Mine",
            StatusKind::Disposed,
            (2030, 11, 26),
            "synthetic disposal source",
        ),
        SubjectStatus::from_date(
            "Example Mine",
            StatusKind::Disposed,
            (2030, 12, 1),
            "synthetic December disposal source",
        ),
        SubjectStatus {
            subject: "Example Mine".into(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 9)),
            evidence: "synthetic withheld source".into(),
        },
    ] {
        let expected = if status.kind == StatusKind::Disposed {
            "definition_changed"
        } else {
            "withheld_target"
        };
        p.subject_statuses = vec![status];
        let r = run(&p, &t, &c);
        for outcome in [&r.rows[0].first_published, &r.rows[0].latest_registered] {
            assert_eq!(outcome.status, expected);
            assert!(outcome.score.is_none());
            assert!(
                outcome
                    .refusal
                    .as_ref()
                    .unwrap()
                    .what
                    .contains("evidence: synthetic")
            );
            assert!(matches!(
                outcome.sources[0].claim.value,
                PublicValue::Number {
                    under_status: None,
                    ..
                }
            ));
        }
        assert_eq!(r.summaries[0].scores.n_members, 2);
        assert_eq!(r.summaries[0].scores.n_unresolved, 2);
    }
    p.subject_statuses = vec![SubjectStatus::from_date(
        "Example Mine",
        StatusKind::Disposed,
        (2031, 12, 1),
        "synthetic later disposal",
    )];
    assert!(run(&p, &t, &c).rows[0].first_published.score.is_some());
    p.subject_statuses = vec![SubjectStatus {
        subject: "Example Mine".into(),
        kind: StatusKind::CareAndMaintenance,
        first: (2030, 1),
        last: None,
        evidence: "synthetic maintenance".into(),
    }];
    assert!(run(&p, &t, &c).rows[0].first_published.score.is_some());
}

static TEMP: AtomicUsize = AtomicUsize::new(0);
fn workspace() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "fmine-track-record-{}-{}",
        std::process::id(),
        TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("corpus")).unwrap();
    std::fs::create_dir_all(root.join("registries/sources")).unwrap();
    std::fs::create_dir_all(root.join("registries/transcriptions")).unwrap();
    let text = |s: &str| MinimalCell::Text(s.into());
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "YTD".into(),
        hidden: false,
        cells: vec![
            ("A1".into(), text("Summary of Operations")),
            (
                "A3".into(),
                text("For the twelve months ended December 31,"),
            ),
            ("B3".into(), MinimalCell::Number(2030.0)),
            ("B4".into(), text("Gold produced (000s oz) (100%)")),
            (
                "C4".into(),
                text("Gold produced (000s oz) (attributable basis)"),
            ),
            ("A5".into(), text("Example Mine (80%)")),
            ("B5".into(), MinimalCell::Number(156.0)),
            ("C5".into(), MinimalCell::Number(125.0)),
            ("A6".into(), text("Missing Mine (80%)")),
            ("B6".into(), text("—")),
            ("C6".into(), text("—")),
        ],
    }]);
    std::fs::write(root.join("corpus/Original.xlsx"), &bytes).unwrap();
    let doc = b"synthetic issuer guidance, 100 - 120; invented fixture, not a public filing";
    std::fs::write(root.join("corpus/Guidance.pdf"), doc).unwrap();
    let sha = sha256(doc).to_hex();
    let registry = format!(
        r#"schema = "fmine.sources.v1"
issuer = "synthetic"
corpus_dir = "corpus"
retrieved_utc = "2031-02-01T00:00:00Z"
url_prefix = "https://example.invalid/"

[[source]]
file = "Original.xlsx"
kind = "mine_stats"
quarter = "2030Q4"
path = "Original.xlsx"
url_basis = "W1 table"
bytes = {}
sha256 = "{}"

[[document]]
file = "Guidance.pdf"
dir = "corpus"
kind = "mda"
url = "https://example.invalid/Guidance.pdf"
url_basis = "W1 table"
published_at = "2030-01-15"
published_at_basis = "FACT: synthetic invented fixture"
pages = 1
bytes = {}
sha256 = "{sha}"

[[subject]]
name = "Example Mine"

[[subject]]
name = "Missing Mine"
"#,
        bytes.len(),
        sha256(&bytes).to_hex(),
        doc.len()
    );
    std::fs::write(root.join("registries/sources/synthetic.toml"), registry).unwrap();
    let mut rows = String::from("schema = \"fmine.transcriptions.v1\"\nissuer = \"synthetic\"\n");
    for (site, key) in [
        ("Example Mine", "example_mine"),
        ("Missing Mine", "missing_mine"),
    ] {
        for reader in ["reader-a", "reader-b"] {
            writeln!(rows,"\n[[entry]]\ndoc = \"Guidance.pdf\"\ndoc_sha256 = \"{sha}\"\npage = 1\ntable = \"Invented guidance\"\nrow = \"{site}\"\ncol = \"Production\"\ntoken = \"100 - 120\"\nquantity = \"guidance_{key}_production_koz\"\nunit = \"koz\"\nbasis = \"attributable:4/5\"\nperiod = \"2030FY\"\nreader = \"{reader}\"").unwrap();
        }
    }
    std::fs::write(root.join("registries/transcriptions/synthetic.toml"), rows).unwrap();
    root
}

#[test]
fn registered_file_path_verifies_bytes_and_retains_placeholder_outcome() {
    let root = workspace();
    let r = track_record::run(&root, "synthetic", None, "2031-02-02").unwrap();
    // Reserved synthetic input remains Unknown; loading it must not promote an invented source to Known.
    assert!(r.rows[0].first_published.score.is_none());
    assert_eq!(
        r.rows[0].first_published.refusal.as_ref().unwrap().code,
        RefusalCode::NotObservable202.as_str()
    );
    assert!(matches!(&r.rows[0].first_published.sources[0].claim.value,
        PublicValue::Number { displayed: Ok(value), .. } if value == "125"));
    assert_eq!(r.summaries[0].scores.n_members, 2);
    assert_eq!(r.summaries[0].scores.n_unresolved, 2);
    assert!(matches!(
        r.rows[1].first_published.sources[0].claim.value,
        PublicValue::Nil { .. }
    ));
    assert_eq!(
        track_record::run(&root, "synthetic", Some("Example Mine"), "2031-02-02")
            .unwrap()
            .rows
            .len(),
        1
    );
    std::fs::write(
        root.join("corpus/Guidance.pdf"),
        b"altered invented fixture",
    )
    .unwrap();
    assert_eq!(
        track_record::run(&root, "synthetic", None, "2031-02-02")
            .unwrap_err()
            .code,
        RefusalCode::DigestMismatch505.as_str()
    );
}
