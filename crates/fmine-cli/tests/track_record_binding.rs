//! BD-341 source-selection regressions. All invented guidance is labelled synthetic; real workbooks
//! are used only in the explicitly ignored source-binding test, never as fabricated guidance evidence.

use fmine_calib::CalibrationFlag;
use fmine_calib::guidance_scores::claims::{Hit, Side};
use fmine_claim::{Claim, Confidentiality};
use fmine_cli::track_record_binding::{self as track_record, TrackRecordInput, TrackRecordReport};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind, Refusal};
use fmine_io::xlsx::Workbook;
use fmine_numerics::exact_feas::Rat;
use fmine_public::key::{self, KeyedCell, OwnershipBasis, PublicValue, StatusKind, SubjectStatus};
use fmine_public::transcribe::TranscriptionReport;
use fmine_registry::transcriptions::parse_token;

fn assess(
    t: &TranscriptionReport,
    cells: &[KeyedCell],
    statuses: &[SubjectStatus],
) -> Result<TrackRecordReport, Refusal> {
    track_record::assess(TrackRecordInput {
        issuer: "synthetic",
        site: None,
        through: None,
        as_of: "2032-02-29",
        available_by: "2032-02-29",
        transcriptions: t,
        claims: cells,
        statuses,
        synthetic: true,
    })
}

fn rebind(template: &KeyedCell, value: PublicValue) -> KeyedCell {
    let workbook = Workbook {
        artifact_path: template.locator.clone(),
        sha256: template.claim.anchors[0].artifact_sha256,
        sheets: vec![],
        hidden_sheets: vec![],
        quarantined: vec![],
    };
    let mut c = key::public_claim(&workbook, "YTD", "C1", template.key.clone(), value);
    c.claim = Claim::new(
        &c.claim.metric_key,
        c.claim.value.clone(),
        c.claim.value_tag(),
        c.claim.representation.clone(),
        c.claim.support,
        c.claim.additivity,
        c.claim.evidence,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        c.claim.confidentiality.clone(),
        c.claim.uu_terms.clone(),
        c.claim.parents.clone(),
        c.claim.anchors.clone(),
    );
    c
}

fn state(cell: &mut KeyedCell, state: KnowledgeState) {
    let c = &cell.claim;
    cell.claim = Claim::new(
        &c.metric_key,
        c.value.clone(),
        c.value_tag(),
        c.representation.clone(),
        c.support,
        c.additivity,
        c.evidence,
        c.epistemic,
        state,
        c.confidentiality.clone(),
        c.uu_terms.clone(),
        c.parents.clone(),
        c.anchors.clone(),
    );
}

#[test]
fn synthetic_label_must_match_the_reserved_issuer_in_both_directions() {
    let (t, _) = track_record::synthetic_evidence().unwrap();
    for (issuer, synthetic) in [("synthetic", false), ("barrick", true)] {
        let refusal = track_record::assess(TrackRecordInput {
            issuer,
            synthetic,
            site: None,
            through: None,
            as_of: "2032-02-29",
            available_by: "2032-02-29",
            transcriptions: &t,
            claims: &[],
            statuses: &[],
        })
        .unwrap_err();
        assert_eq!(refusal.code, "FMINE-REF-105");
        assert!(refusal.what.contains("synthetic lane and issuer disagree"));
    }
}

#[test]
fn even_unselected_non_public_evidence_refuses_before_a_report_can_retain_it() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    cells[3] = rebind(
        &cells[3],
        PublicValue::Number {
            stored: "987654321".into(),
            displayed: Ok("987654321".into()),
            under_status: None,
        },
    );
    cells[3].claim.confidentiality = Confidentiality::internal();
    state(&mut cells[3], KnowledgeState::Estimated);
    cells[3].claim.check_id().unwrap();
    let refusal = track_record::assess(TrackRecordInput {
        issuer: "synthetic",
        synthetic: true,
        site: Some("Example Mine"),
        through: Some("2030Q4"),
        as_of: "2032-02-29",
        available_by: "2032-02-29",
        transcriptions: &t,
        claims: &cells,
        statuses: &[],
    })
    .unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-301");
    assert!(!refusal.what.contains("987654321"));
    assert!(!refusal.what.contains(&cells[3].locator));
}

#[test]
fn synthetic_unknown_source_is_preserved_in_its_interpretation_and_score() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    state(&mut cells[0], KnowledgeState::Unknown);
    let source_id = cells[0].claim.claim_id;
    let r = assess(&t, &cells, &[]).unwrap();
    let row = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
    assert_eq!(row.original.source_claims[0].claim.claim_id, source_id);
    assert_eq!(
        row.original.source_claims[0].claim.state,
        KnowledgeState::Unknown
    );
    assert_eq!(cells[0].claim.state, KnowledgeState::Unknown);
    let calculated = row.original.outcome.as_ref().unwrap();
    assert_eq!(calculated.metric_key, "guidance.interpreted_outcome");
    assert_eq!(calculated.state, KnowledgeState::Unknown);
    assert_eq!(calculated.evidence, EvidenceClass::Assumed);
    assert_eq!(calculated.epistemic, cells[0].claim.epistemic);
    assert_eq!(calculated.parents, vec![source_id]);
    assert_eq!(
        row.original.score.as_ref().unwrap().state,
        KnowledgeState::Unknown
    );
    assert_eq!(
        row.guidance.parents,
        vec![row.guidance_source_claim.claim_id]
    );
    assert!(
        r.to_json()
            .contains(&row.guidance_source_claim.claim_id.to_string())
    );
    for cell in &mut cells {
        cell.key.issuer = "other".into();
        *cell = rebind(cell, cell.claim.value.clone());
        state(cell, KnowledgeState::Unknown);
    }
    let real = track_record::assess(TrackRecordInput {
        issuer: "other",
        site: None,
        through: None,
        as_of: "2032-02-29",
        available_by: "2032-02-29",
        transcriptions: &t,
        claims: &cells,
        statuses: &[],
        synthetic: false,
    })
    .unwrap();
    assert!(real.rows.iter().all(|r| r.original.outcome.is_none()));
}

#[test]
fn negative_production_stays_unresolved_and_exact_binomial_bounds_are_serialized() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    cells[0] = rebind(
        &cells[0],
        PublicValue::Number {
            stored: "-1".into(),
            displayed: Ok("-1".into()),
            under_status: None,
        },
    );
    let r = assess(&t, &cells, &[]).unwrap();
    let row = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
    assert!(row.original.outcome.is_none());
    assert!(
        row.original
            .reason
            .as_ref()
            .unwrap()
            .contains("production cannot be negative")
    );
    assert_eq!(r.summaries[0].summary.value.members, 3);
    assert!(r.summaries[0].summary.value.binomial_90_exact.is_some());
    assert!(r.to_json().contains("\"binomial_90_exact\":{\"lo\":"));
    assert!(r.to_json().contains("guidance.track_record/2"));
}

#[test]
fn dated_synthetic_replay_excludes_future_outcome_appearances() {
    let early = track_record::synthetic_as_of("2030-06-01").unwrap();
    assert_eq!(early.as_of, "2030-06-01");
    assert_eq!(early.rows.len(), 3);
    assert!(
        early
            .rows
            .iter()
            .all(|r| r.original.outcome.is_none() && r.later.is_empty())
    );
    let first = track_record::synthetic_as_of("2031-01-15").unwrap();
    assert_eq!(first.summaries[0].summary.value.resolved, 2);
    assert!(first.rows.iter().all(|r| r.later.is_empty()));
    assert_ne!(early.digest, first.digest);
    assert!(!first.to_json().contains("9999-12-31"));
}

#[test]
fn exact_miss_boundary_and_missing_original_are_distinct() {
    let r = track_record::synthetic().unwrap();
    let example = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
    let original = example.original.score.as_ref().unwrap();
    assert_eq!(original.value.z, Some(Rat::new(3, 2).unwrap()));
    assert_eq!(original.value.side, Side::Above);
    assert_eq!(original.value.hit, Hit::CertainMiss);
    assert_eq!(
        example.later[0].score.as_ref().unwrap().value.z,
        Some(Rat::new(1, 2).unwrap())
    );
    assert_eq!(example.original.release, "2030Q4");
    let boundary = r.rows.iter().find(|r| r.site == "Boundary Mine").unwrap();
    assert_eq!(
        boundary.original.score.as_ref().unwrap().value.hit,
        Hit::Boundary
    );
    let missing = r.rows.iter().find(|r| r.site == "Missing Mine").unwrap();
    assert!(missing.original.outcome.is_none());
    assert_eq!(missing.later[0].kind, "resolved_on_comparative_vintage");
    let s = &r.summaries[0].summary.value;
    assert_eq!((s.members, s.resolved, s.unresolved), (3, 2, 1));
    assert_eq!(
        (s.printed_hits, s.certain_hits, s.certain_misses, s.boundary),
        (1, 0, 1, 1)
    );
    assert_eq!(s.whole_cohort_hit_rate_bounds.lo, Rat::new(1, 3).unwrap());
    assert_eq!(s.whole_cohort_hit_rate_bounds.hi, Rat::new(2, 3).unwrap());
    assert!(r.gate_fired);
    assert_eq!(r.to_json(), track_record::synthetic().unwrap().to_json());
    for row in &r.rows {
        assert_eq!(row.guidance.state, KnowledgeState::Estimated);
        row.guidance.check_id().unwrap();
        if let Some(s) = &row.original.score {
            s.check_id().unwrap();
            assert!(!s.anchors.is_empty());
        }
    }
}

#[test]
fn withholding_preserves_members_and_all_unresolved_is_unscored() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    cells.retain(|c| c.key.reporting_vintage.release != "2030Q4");
    let r = assess(&t, &cells, &[]).unwrap();
    let s = &r.summaries[0].summary.value;
    assert_eq!((s.members, s.resolved, s.unresolved), (3, 0, 3));
    assert_eq!(s.flag, CalibrationFlag::Unscored);
    assert_eq!(s.binomial_90, None);
    assert!(r.to_json().contains("\"flag\":\"unscored\""));
    assert!(r.rows.iter().all(|row| row.original.outcome.is_none()));
}

#[test]
fn attributable_guidance_never_uses_full_basis_or_unprinted_basis() {
    for basis in [OwnershipBasis::Consolidated100, OwnershipBasis::Unprinted] {
        let (t, mut cells) = track_record::synthetic_evidence().unwrap();
        let c = cells
            .iter_mut()
            .find(|c| {
                c.key.subject == "Example Mine" && c.key.reporting_vintage.release == "2030Q4"
            })
            .unwrap();
        c.key.ownership_basis = basis;
        *c = rebind(c, c.claim.value.clone());
        let r = assess(&t, &cells, &[]).unwrap();
        let row = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
        assert!(row.original.outcome.is_none());
        assert!(
            row.original
                .reason
                .as_ref()
                .unwrap()
                .contains("FMINE-REF-105")
        );
        assert!(row.later[0].outcome.is_some());
    }
}

#[test]
fn source_and_transcription_mutations_cannot_reuse_original_identity() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    if let PublicValue::Number { displayed, .. } = &mut cells[0].claim.value {
        *displayed = Ok("999".into());
    }
    let r = assess(&t, &cells, &[]).unwrap();
    let row = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
    assert!(
        row.original
            .reason
            .as_ref()
            .unwrap()
            .contains("FMINE-REF-507")
    );
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    t.agreement.accepted[0].token = "900 - 999".into();
    assert_eq!(assess(&t, &cells, &[]).unwrap_err().code, "FMINE-REF-208");
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    t.documents.push(t.documents[0].clone());
    assert_eq!(assess(&t, &cells, &[]).unwrap_err().code, "FMINE-REF-202");
}

#[test]
fn pending_and_unsupported_guidance_are_visible_without_filling_values() {
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    t.agreement.single_reader.push((
        t.agreement.accepted[0].locator.clone(),
        "pending-reader".into(),
    ));
    let mut unsupported = t.agreement.accepted[0].clone();
    unsupported.quantity = "guidance_example_mine_capex_usd_m".into();
    unsupported.locator.col = "Capital expenditure".into();
    t.agreement.accepted.push(unsupported);
    let r = assess(&t, &cells, &[]).unwrap();
    assert_eq!(r.rows.len(), 3);
    assert_eq!(r.pending_transcriptions.len(), 1);
    assert_eq!(r.unsupported_guidance.len(), 1);
    assert!(r.to_json().contains("unsupported_metric"));
    let mut broken = t.clone();
    broken.agreement.accepted[0].readers.pop();
    assert_eq!(
        assess(&broken, &cells, &[]).unwrap_err().code,
        "FMINE-REF-202"
    );
}

#[test]
fn revisions_do_not_double_weight_the_original_cohort() {
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    let mut doc = t.documents[0].clone();
    doc.file = "revised-guidance.txt".into();
    doc.published_at = "2030-06-01".into();
    let mut revised = t.agreement.accepted[0].clone();
    revised.locator.doc = doc.file.clone();
    revised.token = "110 - 130".into();
    revised.value = parse_token(&revised.token).unwrap();
    t.documents.push(doc);
    t.agreement.accepted.push(revised);
    let r = assess(&t, &cells, &[]).unwrap();
    assert_eq!(r.rows.len(), 4);
    assert_eq!(r.summaries.len(), 2);
    assert_eq!(r.summaries[0].summary.value.members, 3);
    assert_eq!(r.summaries[1].summary.value.members, 1);
    assert_eq!(
        r.rows
            .iter()
            .filter(|r| r.vintage_kind == "revised")
            .count(),
        1
    );
    t.documents[1].published_at = "2030-01-15".into();
    assert_eq!(assess(&t, &cells, &[]).unwrap_err().code, "FMINE-REF-105");
}

#[test]
fn unchanged_target_with_changed_basis_is_revised_before_a_true_reaffirmation() {
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    for (file, date) in [
        ("changed-basis-guidance.txt", "2030-06-01"),
        ("reaffirmed-basis-guidance.txt", "2030-09-01"),
    ] {
        let mut doc = t.documents[0].clone();
        doc.file = file.into();
        doc.published_at = date.into();
        let mut guidance = t.agreement.accepted[0].clone();
        guidance.locator.doc = doc.file.clone();
        guidance.basis = "100%".into();
        t.documents.push(doc);
        t.agreement.accepted.push(guidance);
    }
    let r = assess(&t, &cells, &[]).unwrap();
    let rows: Vec<_> = r.rows.iter().filter(|r| r.site == "Example Mine").collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].guidance.value.target, rows[1].guidance.value.target);
    assert_ne!(
        rows[0].guidance.value.key.basis,
        rows[1].guidance.value.key.basis
    );
    assert_eq!(rows[0].vintage_kind, "initial_registered");
    assert_eq!(rows[1].vintage_kind, "revised");
    assert_eq!(rows[2].vintage_kind, "reaffirmed");
    assert_eq!(rows[1].guidance.value, rows[2].guidance.value);
    assert!(rows[1].original.outcome.is_none());
    assert!(rows[2].original.outcome.is_none());
    assert_eq!(r.summaries.len(), 3);
    assert_eq!(r.summaries[0].summary.value.members, 3);
    assert_eq!(r.summaries[1].summary.value.members, 1);
    assert_eq!(r.summaries[2].summary.value.members, 1);
}

#[test]
fn unusable_original_and_status_overlap_never_resolve_on_a_later_number() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    cells[0] = rebind(&cells[0], PublicValue::Missing);
    let r = assess(&t, &cells, &[]).unwrap();
    assert!(
        r.rows
            .iter()
            .find(|r| r.site == "Example Mine")
            .unwrap()
            .original
            .outcome
            .is_none()
    );
    for status in [
        SubjectStatus::from_date(
            "Example Mine",
            StatusKind::Disposed,
            (2030, 12, 1),
            "synthetic December disposal",
        ),
        SubjectStatus {
            subject: "Example Mine".into(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 9)),
            evidence: "synthetic withholding".into(),
        },
    ] {
        let (_, cells) = track_record::synthetic_evidence().unwrap();
        let r = assess(&t, &cells, &[status]).unwrap();
        let row = r.rows.iter().find(|r| r.site == "Example Mine").unwrap();
        assert!(row.original.outcome.is_none());
        assert!(row.later.iter().all(|a| a.outcome.is_none()));
        assert_eq!(r.summaries[0].summary.value.members, 3);
    }
}

#[test]
fn chronology_and_source_bound_changes_affect_admissibility_and_digest() {
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    let original = assess(&t, &cells, &[]).unwrap().digest;
    t.documents[0].published_at_basis = "different supplied date evidence".into();
    assert_ne!(original, assess(&t, &cells, &[]).unwrap().digest);
    t.documents[0].published_at = "2031-01-01".into();
    let r = assess(&t, &cells, &[]).unwrap();
    assert_eq!(r.summaries[0].summary.value.resolved, 0);
    assert!(r.rows.iter().all(|r| {
        r.original
            .reason
            .as_ref()
            .unwrap()
            .contains("before the annual period closed")
    }));
    let (t, cells) = track_record::synthetic_evidence().unwrap();
    let r = track_record::assess(TrackRecordInput {
        issuer: "synthetic",
        site: None,
        through: Some("2030Q4"),
        as_of: "2032-02-29",
        available_by: "2032-02-29",
        transcriptions: &t,
        claims: &cells,
        statuses: &[],
        synthetic: true,
    })
    .unwrap();
    assert!(r.rows.iter().all(|r| r.later.is_empty()));
    assert_ne!(r.digest, original);
    assert!(
        track_record::assess(TrackRecordInput {
            issuer: "synthetic",
            site: None,
            through: None,
            as_of: "2030-01-01",
            available_by: "2032-02-29",
            transcriptions: &t,
            claims: &cells,
            statuses: &[],
            synthetic: true
        })
        .is_err()
    );
}

#[test]
fn input_order_does_not_change_the_projection() {
    let (mut t, mut cells) = track_record::synthetic_evidence().unwrap();
    let original = assess(&t, &cells, &[]).unwrap();
    t.agreement.accepted.reverse();
    cells.reverse();
    t.documents.reverse();
    assert_eq!(
        original.to_json(),
        assess(&t, &cells, &[]).unwrap().to_json()
    );
}

#[test]
fn point_guidance_has_no_invented_normalized_miss() {
    let (mut t, cells) = track_record::synthetic_evidence().unwrap();
    for a in &mut t.agreement.accepted {
        a.token = "120".into();
        a.value = parse_token(&a.token).unwrap();
    }
    let r = assess(&t, &cells, &[]).unwrap();
    assert!(
        r.rows
            .iter()
            .filter_map(|r| r.original.score.as_ref())
            .all(|s| s.value.z.is_none())
    );
    assert_eq!(r.summaries[0].summary.value.z_members, 0);
}

#[test]
fn another_issuers_identically_named_site_cannot_supply_an_outcome() {
    let (t, mut cells) = track_record::synthetic_evidence().unwrap();
    cells[0].key.issuer = "another-issuer".into();
    assert_eq!(assess(&t, &cells, &[]).unwrap_err().code, "FMINE-REF-105");
}

#[test]
#[ignore = "requires the hash-verified registered Barrick workbooks"]
fn real_annual_bindings_keep_originals_comparatives_copper_units_and_disposals_distinct() {
    use fmine_core::sha256;
    use fmine_public::transcribe::CheckedDocument;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = fmine_public::load_issuer_profile(&root, "barrick").unwrap();
    let claims = key::mine_stats::load_claims(&profile, &root).unwrap();
    // Explicit assumed copies keep this hypothetical experiment in the reserved synthetic namespace.
    // Every source anchor survives, and each copy names the untouched authenticated Claim as its parent.
    let synthetic_cells: Vec<_> = claims
        .claims
        .iter()
        .map(|source| {
            let mut copy = source.clone();
            copy.key.issuer = "synthetic".into();
            let tagged = rebind(&copy, copy.claim.value.clone());
            let c = &source.claim;
            copy.claim = Claim::new(
                &c.metric_key,
                c.value.clone(),
                tagged.claim.value_tag(),
                c.representation.clone(),
                c.support,
                c.additivity,
                EvidenceClass::Assumed,
                EpistemicStatus::KnownUnknown(KuKind::Parametric),
                if c.state == KnowledgeState::Known {
                    KnowledgeState::Estimated
                } else {
                    c.state
                },
                c.confidentiality.clone(),
                c.uu_terms.clone(),
                vec![c.claim_id],
                c.anchors.clone(),
            );
            copy
        })
        .collect();
    let (mut t, _) = track_record::synthetic_evidence().unwrap();
    let template = t.agreement.accepted[0].clone();
    t.agreement.accepted.clear();
    t.documents.clear();
    for year in [2023, 2024, 2025] {
        let doc = format!("synthetic-guidance-{year}.txt");
        let hash = sha256(doc.as_bytes()).to_hex();
        t.documents.push(CheckedDocument {
            file: doc.clone(),
            kind: "synthetic".into(),
            published_at: format!("{year}-01-15"),
            published_at_basis: "invented; not an issuer guidance assertion".into(),
            sha256: hash.clone(),
        });
        for (site, metric, unit, basis) in [
            ("North Mara", "production_koz", "koz", "attributable:21/25"),
            (
                "North Mara",
                "aisc_usd_per_oz",
                "usd_per_oz",
                "attributable:21/25;aisc=issuer-reported-per-sold-oz",
            ),
            ("Lumwana", "production_kt", "kt", "100%"),
            (
                "Lumwana",
                "aisc_usd_per_lb",
                "usd_per_lb",
                "100%;aisc=issuer-reported-per-sold-lb",
            ),
            ("Hemlo", "production_koz", "koz", "100%"),
            ("Tongon", "production_koz", "koz", "attributable:897/1000"),
            (
                "Loulo-Gounkoto",
                "production_koz",
                "koz",
                "attributable:4/5",
            ),
        ] {
            let mut a = template.clone();
            a.locator.doc = doc.clone();
            a.locator.row = site.into();
            a.locator.col = metric.into();
            a.doc_sha256 = hash.clone();
            a.quantity = format!(
                "guidance_{}_{}",
                site.to_ascii_lowercase().replace([' ', '-'], "_"),
                metric
            );
            a.unit = unit.into();
            a.basis = basis.into();
            a.period = format!("{year}FY");
            t.agreement.accepted.push(a);
        }
    }
    // These are hypothetical targets only. The test verifies real source binding, never actual issuer guidance.
    let r = track_record::assess(TrackRecordInput {
        issuer: "synthetic",
        site: None,
        through: Some("2026Q2"),
        as_of: "2026-11-01",
        available_by: "2026-11-01",
        transcriptions: &t,
        claims: &synthetic_cells,
        statuses: &profile.subject_statuses,
        synthetic: true,
    })
    .unwrap();
    assert!(r.synthetic);
    assert!(
        r.rows
            .iter()
            .all(|r| r.guidance.evidence == EvidenceClass::Assumed)
    );
    for row in &r.rows {
        if row.year == 2023 {
            assert!(row.original.outcome.is_none());
            assert!(
                row.later
                    .iter()
                    .all(|a| a.kind == "resolved_on_comparative_vintage")
            );
        }
        if row.year == 2025 && ["Hemlo", "Tongon", "Loulo-Gounkoto"].contains(&row.site.as_str()) {
            assert!(row.original.outcome.is_none(), "{}", row.site);
        }
        if row.year >= 2024 && row.site == "Lumwana" && row.metric == "aisc_usd_per_lb" {
            // The source omits ownership on this cost row; a separate ownership bridge is required.
            assert_eq!(
                row.original.source_claims[0].key.ownership_basis,
                OwnershipBasis::Unprinted
            );
            assert!(row.original.outcome.is_none());
            assert!(
                row.original
                    .reason
                    .as_ref()
                    .unwrap()
                    .contains("FMINE-REF-105")
            );
        } else if row.year >= 2024 && ["North Mara", "Lumwana"].contains(&row.site.as_str()) {
            assert!(
                row.original.outcome.is_some(),
                "{} {} {}: {:?}",
                row.site,
                row.year,
                row.metric,
                (
                    row.original.reason.clone(),
                    row.original
                        .source_claims
                        .iter()
                        .map(|c| (&c.key.unit, c.key.ownership_basis))
                        .collect::<Vec<_>>()
                )
            );
            let original = &row.original.source_claims[0];
            assert_eq!(
                original.key.reporting_vintage.release,
                format!("{}Q4", row.year)
            );
            assert_eq!(original.key.unit, row.unit);
            assert_eq!(
                row.original.outcome.as_ref().unwrap().parents,
                vec![original.claim.claim_id]
            );
            assert!(
                claims
                    .claims
                    .iter()
                    .any(|c| original.claim.parents.contains(&c.claim.claim_id)
                        && c.claim.anchors == original.claim.anchors)
            );
            assert_ne!(original.claim.anchors[0].artifact_sha256, [0; 32]);
        }
    }
    assert_eq!(r.rows.len(), 21);
    assert_eq!(
        r.summaries
            .iter()
            .map(|s| s.summary.value.members)
            .sum::<usize>(),
        21
    );
}
