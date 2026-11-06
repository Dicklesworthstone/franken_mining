use super::*;
use fmine_claim::{Claim, Confidentiality};
use fmine_core::{anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{EpistemicStatus, KuKind};
use fmine_public::key::{LineKey, Period, PeriodKind, PublicKey, Vintage};
use fmine_public::sources::RegisteredSource;

#[test]
fn synthetic_assumed_royalties_never_override_negative_knowledge_states() {
    let source_bytes = b"invented Q1 2024 royalty: 50 USD per ounce";
    let source_digest = sha256(source_bytes);
    let file = "Synthetic_Royalty_2024Q1.xlsx";
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.sources.push(RegisteredSource {
        file_name: file.into(),
        kind: SourceKind::MineStats,
        quarter: "2024Q1".into(),
        url: format!("https://example.invalid/{file}"),
        url_basis: "DESIGN: invented in-memory source".into(),
        bytes: source_bytes.len() as u64,
        sha256_hex: source_digest.to_hex(),
    });
    let key = PublicKey {
        issuer: profile.issuer.clone(),
        subject: "Site A".into(),
        valid_period: Period {
            year: 2024,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: "2024Q1".into(),
            file_sha256: source_digest.to_hex(),
        },
        ownership_basis: OwnershipBasis::Consolidated100,
        scope: Scope::Site,
        unit: "usd_per_oz".into(),
        line: LineKey {
            label: "site:costs per ounce:royalties".into(),
            occurrence: 0,
        },
    };
    let cell_for_state = |state| {
        let value = PublicValue::Number {
            stored: "50".into(),
            displayed: Ok("50".into()),
            under_status: None,
        };
        let value_tag = canonical_value_tag(&key, &value);
        let claim = Claim::new(
            &key.line.label,
            value,
            &value_tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Assumed,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            state,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new(file, source_digest.0, "QTD!B7")],
        );
        KeyedCell {
            locator: format!("{file}:QTD!B7"),
            key: key.clone(),
            claim,
            printed_basis: key.ownership_basis,
            basis_evidence: None,
            status_evidence: None,
            label_as_printed: None,
            unit_evidence: None,
        }
    };
    for state in [
        KnowledgeState::Known,
        KnowledgeState::Estimated,
        KnowledgeState::Unknown,
    ] {
        let cell = cell_for_state(state);
        cell.claim.check_id().expect("new canonical source Claim");
        let cited = checked_cell(
            &profile,
            &cell,
            "usd_per_oz",
            "100%",
            price::RateUnit::UsdPerTroyOunce,
        )
        .unwrap_or_else(|error| panic!("{state:?}: {error}"));
        assert_eq!(cited.printed.value, Rat::from_integer(50));
        assert_eq!(cited.citation.locator, cell.locator);
        assert_eq!(cited.citation.sha256, source_digest);
    }
    for state in [
        KnowledgeState::Conflicted,
        KnowledgeState::NotObservable,
        KnowledgeState::Redacted,
        KnowledgeState::Stale,
        KnowledgeState::Indeterminate,
        KnowledgeState::NotApplicable,
    ] {
        let cell = cell_for_state(state);
        // Rehash the changed state so failure cannot come from a stale Claim id.
        cell.claim.check_id().expect("new canonical source Claim");
        let error = checked_cell(
            &profile,
            &cell,
            "usd_per_oz",
            "100%",
            price::RateUnit::UsdPerTroyOunce,
        )
        .expect_err("synthetic evidence must preserve an explicit negative state");
        assert_eq!(
            error, "source is not an observable numeric claim",
            "{state:?}"
        );
    }
}
