//! Cross-crate Mine Stats Claim inventory contracts (BD-332.S3, BD-115).
//! Kept in the T layer with unchanged assertions as the public waterfall joins the production crate.
#![cfg(test)]

use fmine_core::hash::Digest32;
use fmine_public::LoadedSource;
use fmine_public::key::mine_stats::{KeyedClaims, build_claims, load_claims, value_kind};
use fmine_public::key::{KeyedCell, NilReason, PublicValue, claims_digest, from_mine_stats};
use fmine_public::mine_stats::{CellRefusal, MineStatsRead};
use fmine_public::sources::{IssuerProfile, SourceKind};
pub use fmine_public::{key, load_registered_sources, mine_stats, sources, synthetic};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{OwnershipBasis, ShareSource, StatusKind, SubjectStatus, raw_token};
    use crate::mine_stats::{MineStatsCell, read_mine_stats};
    use crate::sources::{PrintedShareCorrection, RegisteredSource, load_issuer_profile};
    use fmine_evidence::EvidenceClass;
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn fixture_bytes(raw: MinimalCell) -> Vec<u8> {
        let text = |s: &str| MinimalCell::Text(s.into());
        build_minimal_xlsx(&[MinimalSheet {
            name: "Minestats1".into(),
            hidden: false,
            cells: vec![
                ("A1".into(), text("Mine Statistics (Gold)")),
                ("B2".into(), text("Site A (61.5%)1")),
                ("A3".into(), text("For the three months ended March 31,")),
                ("B3".into(), MinimalCell::Number(2030.0)),
                ("A4".into(), text("Recovery rate (percent)")),
                ("B4".into(), raw),
            ],
        }])
    }

    fn fixture(raw: MinimalCell) -> (LoadedSource, MineStatsRead, IssuerProfile) {
        let bytes = fixture_bytes(raw);
        let workbook = Workbook::from_bytes("synthetic.xlsx", &bytes).expect("workbook");
        let read = read_mine_stats(&workbook, "2030Q1", None).expect("read");
        assert!(read.refusals.is_empty(), "{:?}", read.refusals);
        assert_eq!(read.values.len(), 1);
        let mut profile = crate::synthetic::synthetic_profile();
        profile.sources.push(RegisteredSource {
            file_name: "synthetic.xlsx".into(),
            kind: SourceKind::MineStats,
            quarter: "2030Q1".into(),
            url: "https://example.invalid/synthetic.xlsx".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            sha256_hex: Digest32(workbook.sha256).to_hex(),
        });
        profile.subject_statuses = vec![SubjectStatus::from_date(
            "Site A",
            StatusKind::Disposed,
            (2029, 12, 1),
            "synthetic-status",
        )];
        let source = LoadedSource {
            file_name: "synthetic.xlsx".into(),
            kind: SourceKind::MineStats,
            quarter: "2030Q1".into(),
            url: "https://example.invalid/synthetic.xlsx".into(),
            url_basis: "synthetic".into(),
            bytes: bytes.len() as u64,
            census: workbook.cell_census(),
            workbook,
        };
        (source, read, profile)
    }

    fn inventory(p: &IssuerProfile, s: &LoadedSource, read: &MineStatsRead) -> KeyedClaims {
        build_claims(
            p,
            std::slice::from_ref(s),
            &[(s.file_name.clone(), read.clone())],
        )
    }

    #[test]
    fn inventory_keeps_canonical_ids_values_anchors_and_digest() {
        let (source, read, p) = fixture(MinimalCell::Number(91.0));
        let all = inventory(&p, &source, &read);
        assert!(all.refusals.is_empty());
        assert_eq!(all.claims.len(), 1);
        let canonical = from_mine_stats(&p, &source.workbook, &source.quarter, &read.values[0])
            .expect("canonical claim");
        assert_eq!(all.claims[0].to_json(), canonical.to_json());
        assert_eq!(all.digest, claims_digest([&canonical]));
        assert_eq!(all.claims[0].key.unit, "percent");
        assert_eq!(all.claims[0].locator, "synthetic.xlsx:Minestats1!B4");
        assert_eq!(
            all.claims[0].claim.anchors,
            vec![source.workbook.make_anchor("Minestats1", "B4")]
        );
        assert!(
            all.claims[0]
                .to_json()
                .contains("\"evidence\":\"synthetic\"")
        );
        assert!(all.summary_json().contains("\"number\":1"));
    }

    #[test]
    fn inventory_preserves_status_and_registered_basis_interpretation() {
        let (source, read, mut p) = fixture(MinimalCell::Number(0.0));
        let v = &read.values[0];
        assert!(matches!(v.value, MineStatsCell::Number { value: 0.0, .. }));
        let all = inventory(&p, &source, &read);
        let claim = &all.claims[0];
        assert_eq!(
            claim.claim.value,
            PublicValue::Nil {
                token: "0".into(),
                reason: NilReason::Disposed
            }
        );
        assert_eq!(claim.key.unit, "percent");
        assert_eq!(
            claim.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 123,
                den: 200,
                source: ShareSource::Printed
            }
        );
        assert_eq!(claim.claim.evidence, EvidenceClass::Assumed);
        assert_eq!(claim.status_evidence.as_deref(), Some("synthetic-status"));
        p.subject_statuses.clear();
        let without = inventory(&p, &source, &read);
        assert!(matches!(
            without.claims[0].claim.value,
            PublicValue::Number { .. }
        ));
        assert_ne!(claim.claim.claim_id, without.claims[0].claim.claim_id);
        assert_ne!(all.digest, without.digest);
        p.printed_share_corrections.push(PrintedShareCorrection {
            subject: "Site A".into(),
            printed: "61.5".into(),
            num: 1,
            den: 2,
            evidence: "synthetic-correction".into(),
        });
        let corrected_inventory = inventory(&p, &source, &read);
        let corrected = &corrected_inventory.claims[0];
        assert_eq!(corrected.printed_basis, claim.key.ownership_basis);
        assert_eq!(
            corrected.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Registry
            }
        );
        assert_ne!(corrected.claim.claim_id, without.claims[0].claim.claim_id);
        assert_eq!(
            corrected.basis_evidence.as_deref(),
            Some("synthetic-correction")
        );
    }

    #[test]
    fn source_binding_failures_and_reader_refusals_stay_in_the_inventory() {
        let (source, read, p) = fixture(MinimalCell::Number(91.0));
        let reads = [(source.file_name.clone(), read.clone())];
        let missing = build_claims(&p, &[], &reads);
        assert!(missing.claims.is_empty());
        assert_eq!(missing.refusals.len(), 1);
        assert_eq!(missing.refusals[0].locator, "synthetic.xlsx:Minestats1!B4");
        assert!(missing.refusals[0].reason.contains("no verified source"));

        let mut wrong_read = read.clone();
        wrong_read.values[0].value = MineStatsCell::Number {
            raw_text: "92".into(),
            value: 92.0,
        };
        let mismatched = inventory(&p, &source, &wrong_read);
        assert!(mismatched.claims.is_empty());
        assert_eq!(mismatched.refusals.len(), 1);
        assert!(
            mismatched.refusals[0]
                .reason
                .contains("read does not match")
        );

        let mut wrong_registry = p.clone();
        wrong_registry.sources[0].sha256_hex = "00".repeat(32);
        let mismatched = inventory(&wrong_registry, &source, &read);
        assert!(mismatched.claims.is_empty());
        assert_eq!(mismatched.refusals.len(), 1);
        assert!(
            mismatched.refusals[0]
                .reason
                .contains("do not match the registry")
        );

        let mut refused_read = read;
        refused_read.values[0].section = Some("Unregistered measurement".into());
        refused_read.refusals.push(CellRefusal {
            locator: "2030Q1:Minestats1!A9".into(),
            reason: "synthetic reader refusal".into(),
        });
        let refused = inventory(&p, &source, &refused_read);
        assert!(refused.claims.is_empty());
        assert_eq!(refused.refusals.len(), 2);
        assert!(refused.refusals.contains(&refused_read.refusals[0]));
        assert_eq!(
            refused.digest,
            claims_digest(std::iter::empty::<&KeyedCell>())
        );
    }

    #[test]
    fn loaded_inventory_matches_direct_keys_and_refuses_missing_or_altered_sources() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let ws = std::env::temp_dir().join(format!(
            "fmine-claim-inventory-{}-{nonce}",
            std::process::id()
        ));
        let (_, _, mut p) = fixture(MinimalCell::Number(91.0));
        p.corpus_dir = "corpus".into();
        let corpus = ws.join(&p.corpus_dir);
        std::fs::create_dir_all(&corpus).expect("synthetic workspace");
        std::fs::write(
            corpus.join("synthetic.xlsx"),
            fixture_bytes(MinimalCell::Number(91.0)),
        )
        .expect("synthetic workbook");
        let loaded = load_claims(&p, &ws).expect("loaded claims");
        assert!(!loaded.claims.is_empty());
        let sources = crate::load_registered_sources(&p, &ws).expect("verified files");
        let mut canonical = Vec::new();
        for source in sources.iter().filter(|s| s.kind == SourceKind::MineStats) {
            let read = read_mine_stats(&source.workbook, &source.quarter, None).expect("read");
            canonical.extend(read.values.iter().map(|v| {
                from_mine_stats(&p, &source.workbook, &source.quarter, v).expect("canonical key")
            }));
        }
        assert_eq!(loaded.claims.len(), canonical.len());
        assert_eq!(loaded.digest, claims_digest(&canonical));
        let mut altered = p.clone();
        altered.sources[0].sha256_hex = "00".repeat(32);
        assert_eq!(
            load_claims(&altered, &ws).expect_err("digest").code,
            "FMINE-REF-505"
        );
        let mut missing = p;
        missing.sources[0].file_name = "absent.xlsx".into();
        assert_eq!(
            load_claims(&missing, &ws).expect_err("missing").code,
            "FMINE-REF-506"
        );
    }

    #[test]
    #[ignore = "needs the registered Barrick workbooks"]
    fn barrick_mine_stats_claims_known_answers() {
        let p = load_issuer_profile(&root(), "barrick")
            .expect("profile")
            .through("2026Q2")
            .expect("bound");
        let sources = crate::load_registered_sources(&p, &root()).expect("verified files");
        let vocab = p.mine_stats_labels.iter().cloned().collect();
        let reads: Vec<_> = sources
            .iter()
            .filter(|s| s.kind == SourceKind::MineStats)
            .map(|s| {
                (
                    s.file_name.clone(),
                    read_mine_stats(&s.workbook, &s.quarter, Some(&vocab)).expect("read"),
                )
            })
            .collect();
        let all = build_claims(&p, &sources, &reads);
        assert!(all.refusals.is_empty(), "{:?}", all.refusals);
        assert_eq!(
            all.claims.len(),
            reads.iter().map(|(_, r)| r.values.len()).sum::<usize>()
        );
        // FACT: independent openpyxl read of the registered files and status rows (BD-332.S3), captured
        // before these assertions. These counts are value classifications, not calibration or gate results.
        let mut counts = BTreeMap::new();
        for c in &all.claims {
            *counts.entry(value_kind(&c.claim.value)).or_insert(0_usize) += 1;
        }
        assert_eq!(
            counts,
            BTreeMap::from([
                ("number", 18_768),
                ("nil_unknown", 1_173),
                ("nil_disposed", 7),
                ("not_applicable", 2_778),
                ("missing", 6),
            ])
        );
        let at = |locator: &str| {
            let (release, cell) = locator.split_once(':').expect("oracle locator");
            let file = p
                .sources
                .iter()
                .find(|s| s.kind == SourceKind::MineStats && s.quarter == release)
                .expect("registered Mine Stats release");
            let canonical = format!("{}:{cell}", file.file_name);
            all.claims
                .iter()
                .find(|c| c.locator == canonical)
                .expect(locator)
        };
        for locator in [
            "2026Q1:Minestats3!F19",
            "2026Q2:Minestats3!F19",
            "2026Q2:Minestats3!F64",
            "2026Q1:Minestats1!N27",
            "2026Q1:Minestats1!N28",
            "2026Q2:Minestats1!N27",
            "2026Q2:Minestats1!N28",
        ] {
            assert_eq!(
                at(locator).claim.value,
                PublicValue::Nil {
                    token: "0".into(),
                    reason: NilReason::Disposed
                },
                "{locator}"
            );
        }
        let first = at("2024Q1:QTD!G23");
        let later = at("2025Q1:QTD!N22");
        assert_eq!(raw_token(&first.claim.value), "1479");
        assert_eq!(raw_token(&later.claim.value), "1474");
        assert_eq!(first.key.series_key(), later.key.series_key());
        assert_ne!(first.claim.claim_id, later.claim.claim_id);
        let stale_label = at("2024Q2:QTD!C15");
        assert_eq!(
            stale_label.printed_basis,
            OwnershipBasis::Attributable {
                num: 19,
                den: 40,
                source: ShareSource::Printed
            }
        );
        assert_eq!(
            stale_label.key.ownership_basis,
            OwnershipBasis::Attributable {
                num: 49,
                den: 200,
                source: ShareSource::Registry
            }
        );
        assert_eq!(stale_label.basis_evidence.as_deref(), Some("W1-DR-01"));
        let after = at("2026Q2:YTD!E11");
        assert_eq!(raw_token(&after.claim.value), "1691");
        assert!(matches!(
            after.claim.value,
            PublicValue::Number {
                under_status: Some(StatusKind::Disposed),
                ..
            }
        ));
        let mut reversed = reads.clone();
        reversed.reverse();
        assert_eq!(build_claims(&p, &sources, &reversed).digest, all.digest);
        println!("{}", all.summary_json());
    }
}
