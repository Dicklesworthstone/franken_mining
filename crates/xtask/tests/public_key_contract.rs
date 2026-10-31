//! Public key contracts crossing the claim, evidence, RNG and registry crates.
use fmine_claim::{AdditivityClass, Claim, ConfidentialityTier, Representation, SpatialSupport};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_public::key::{
    LineKey, OwnershipBasis, Period, PeriodKind, PublicKey, Scope, ShareSource, StatusKind,
    SubjectStatus, Vintage, status_for,
};
use std::collections::BTreeMap;

fn p(year: u16, kind: PeriodKind) -> Period {
    Period { year, kind }
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The claim id differs whenever any key field or the raw token differs: 10⁴ (key, token) pairs drawn from
/// small alphabets that include the tag's own separators, so near-collisions are common.
#[test]
fn test_property_claim_id_injective_on_key_and_token() {
    let mut rng = PhiloxStream::new(RngKey {
        seed_root: 332,
        partition: 1,
        domain: 0,
        var_group: 0,
        realization: 0,
        brick: 0,
    });
    let alphabet = ["", "a", "b", "|", ":", "1:", "a|b", "|1:a"];
    let mut pick = |n: usize| (rng.next_u64() % n as u64) as usize;
    let mut seen: BTreeMap<String, (PublicKey, String)> = BTreeMap::new();
    let mut ids = std::collections::BTreeSet::new();
    for _ in 0..10_000 {
        let s = |i: usize| alphabet[i].to_string();
        let kinds = [
            PeriodKind::Quarter(1),
            PeriodKind::Quarter(2),
            PeriodKind::H1,
            PeriodKind::FullYear,
        ];
        let bases = [
            OwnershipBasis::Consolidated100,
            OwnershipBasis::Unprinted,
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Printed,
            },
            OwnershipBasis::Attributable {
                num: 1,
                den: 2,
                source: ShareSource::Registry,
            },
        ];
        let key = PublicKey {
            issuer: s(pick(3)),
            subject: s(pick(8)),
            valid_period: p(2024 + pick(2) as u16, kinds[pick(4)]),
            reporting_vintage: Vintage {
                release: s(pick(4)),
                file_sha256: s(pick(3)),
            },
            ownership_basis: bases[pick(4)],
            scope: match pick(3) {
                0 => Scope::Site,
                1 => Scope::Company,
                _ => Scope::Region {
                    map_version: s(pick(8)),
                },
            },
            unit: s(pick(8)),
            line: LineKey {
                label: s(pick(8)),
                occurrence: pick(2) as u32,
            },
        };
        let raw = s(pick(8));
        let tag = key.canonical_tag(&raw);
        if let Some(prev) = seen.get(&tag) {
            assert_eq!(prev, &(key.clone(), raw.clone()), "tag collision: {tag}");
            continue;
        }
        let claim = Claim::new(
            "public.aisc",
            0_i64,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Derived,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            ConfidentialityTier::Public,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        assert!(ids.insert(claim.claim_id), "claim id collision: {tag}");
        seen.insert(tag, (key, raw));
    }
    // The draws revisit pairs, and every distinct pair got its own tag and id.
    assert!(seen.len() > 5_000, "{}", seen.len());
    assert_eq!(ids.len(), seen.len());
}

/// The registered statuses, hand-written from E486–E489 and E818, and what each covers.
#[test]
fn test_subject_status_registry_golden() {
    let profile = fmine_public::sources::load_issuer_profile(&workspace_root(), "barrick")
        .expect("barrick registry loads");
    let row = |subject: &str, kind, first, last, evidence: &str| SubjectStatus {
        subject: subject.to_string(),
        kind,
        first,
        last,
        evidence: evidence.to_string(),
    };
    assert_eq!(
        profile.subject_statuses,
        vec![
            row(
                "Hemlo",
                StatusKind::Disposed,
                (2025, 12),
                None,
                "E486, E488"
            ),
            row(
                "Tongon",
                StatusKind::Disposed,
                (2026, 1),
                None,
                "E487, E488"
            ),
            row(
                "Loulo-Gounkoto",
                StatusKind::Withheld,
                (2025, 1),
                Some((2025, 9)),
                "E818"
            ),
            row(
                "Long Canyon",
                StatusKind::CareAndMaintenance,
                (2024, 1),
                None,
                "E489"
            ),
        ]
    );
    let s = &profile.subject_statuses;
    let q = |y, n| p(y, PeriodKind::Quarter(n));
    for (subject, period, want) in [
        ("Hemlo", q(2025, 4), None),
        ("Hemlo", p(2025, PeriodKind::FullYear), None),
        ("Hemlo", q(2026, 1), Some(StatusKind::Disposed)),
        ("Hemlo", p(2026, PeriodKind::H1), Some(StatusKind::Disposed)),
        ("Tongon", q(2025, 4), None),
        ("Tongon", q(2026, 2), Some(StatusKind::Disposed)),
        ("Loulo-Gounkoto", q(2024, 4), None),
        ("Loulo-Gounkoto", q(2025, 1), Some(StatusKind::Withheld)),
        (
            "Loulo-Gounkoto",
            p(2025, PeriodKind::NineMonths),
            Some(StatusKind::Withheld),
        ),
        ("Loulo-Gounkoto", q(2025, 4), None),
        ("Long Canyon", q(2023, 4), None),
        (
            "Long Canyon",
            q(2026, 2),
            Some(StatusKind::CareAndMaintenance),
        ),
        ("Kibali", q(2026, 2), None),
    ] {
        assert_eq!(
            status_for(s, subject, &period),
            want,
            "{subject} {period:?}"
        );
    }
}

#[test]
fn test_units_registry_has_usd_m_usd_per_oz() {
    let catalog = fmine_registry::RegistryCatalog::load_from_root(&workspace_root())
        .expect("registries load");
    for code in ["usd_m", "usd_per_oz"] {
        assert!(
            catalog.unit_symbols.iter().any(|u| u == code),
            "{code} not registered"
        );
    }
}
