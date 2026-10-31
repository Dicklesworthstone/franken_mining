//! Source verification and semantic scope binding through the public disclosure API.

use fmine_core::hash::Digest32;
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::LoadedSource;
use fmine_public::key::disclosure::VerifiedDisclosureRegistry;
use fmine_public::key::{
    KeyedCell, LineKey, OwnershipBasis, Period, PeriodKind, PublicKey, PublicValue, Scope, Vintage,
    public_claim,
};
use fmine_public::sources::{
    IssuerProfile, RegisteredSource, RestatementNoteRow, RestatementScopeRow, SourceKind,
};
use fmine_public::synthetic::synthetic_profile;

#[allow(clippy::expect_used)]
fn fixture() -> (IssuerProfile, LoadedSource, KeyedCell) {
    let quote = "Comparative production for Site A has been restated.";
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: "Notes".into(),
        hidden: false,
        cells: vec![
            ("A1".into(), MinimalCell::Text(quote.into())),
            ("B1".into(), MinimalCell::Number(42.0)),
        ],
    }]);
    let workbook = Workbook::from_bytes("notes.xlsx", &bytes).expect("fixture workbook");
    let sha256 = Digest32(workbook.sha256).to_hex();
    let key = PublicKey {
        issuer: "synthetic".into(),
        subject: "Site A".into(),
        valid_period: Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        },
        reporting_vintage: Vintage {
            release: "2030Q2".into(),
            file_sha256: sha256.clone(),
        },
        ownership_basis: OwnershipBasis::Consolidated100,
        scope: Scope::Site,
        unit: "koz".into(),
        line: LineKey {
            label: "summary:au_prod_100_koz".into(),
            occurrence: 0,
        },
    };
    let cell = public_claim(
        &workbook,
        "Notes",
        "B1",
        key,
        PublicValue::Number {
            stored: "42".into(),
            displayed: Ok("42".into()),
            under_status: None,
        },
    );
    let loaded = LoadedSource {
        file_name: "notes.xlsx".into(),
        kind: SourceKind::MineStats,
        quarter: "2030Q2".into(),
        url: "https://example.invalid/notes.xlsx".into(),
        url_basis: "W1 table".into(),
        bytes: bytes.len() as u64,
        census: workbook.cell_census(),
        workbook,
    };
    let mut profile = synthetic_profile();
    profile.sources = vec![RegisteredSource {
        file_name: loaded.file_name.clone(),
        kind: loaded.kind,
        quarter: loaded.quarter.clone(),
        url: loaded.url.clone(),
        url_basis: loaded.url_basis.clone(),
        bytes: loaded.bytes,
        sha256_hex: sha256.clone(),
    }];
    profile.restatement_notes = vec![RestatementNoteRow {
        id: "SYNTHETIC-RESTATEMENT".into(),
        file_name: "notes.xlsx".into(),
        locator: "Notes!A1".into(),
        quoted_text: quote.into(),
        from_quarter: "2030Q2".into(),
        evidence: "synthetic".into(),
        sha256,
    }];
    profile.restatement_scopes = vec![RestatementScopeRow {
        note: "SYNTHETIC-RESTATEMENT".into(),
        subject: cell.key.subject.clone(),
        period_from: "2030Q1".into(),
        period_to: "2030Q4".into(),
        line: cell.key.line.label.clone(),
        occurrence: 0,
    }];
    (profile, loaded, cell)
}

#[test]
fn verified_note_matches_exact_scope_and_never_a_future_note() {
    let (profile, loaded, cell) = fixture();
    let registry = VerifiedDisclosureRegistry::verify(&profile, &[loaded], [&cell]);
    assert!(registry.is_complete());
    let mut first = cell.key.clone();
    first.reporting_vintage.release = "2030Q1".into();
    let citations = registry.matching(&first, &cell.key);
    assert_eq!(citations.len(), 1);
    assert_eq!(citations[0].at, "notes.xlsx:Notes!A1");
    assert_eq!(citations[0].sha256, profile.sources[0].sha256_hex);
    assert!(citations[0].to_json().contains("\"unit\":\"koz\""));
    assert!(
        registry.matching(&first, &first).is_empty(),
        "future note must not leak backward"
    );
    for key in [
        PublicKey {
            issuer: "another".into(),
            ..cell.key.clone()
        },
        PublicKey {
            unit: "kt".into(),
            ..cell.key.clone()
        },
        PublicKey {
            subject: "Site B".into(),
            ..cell.key.clone()
        },
        PublicKey {
            line: LineKey {
                occurrence: 1,
                ..cell.key.line.clone()
            },
            ..cell.key.clone()
        },
    ] {
        assert!(registry.matching(&key, &key).is_empty());
    }
}

#[test]
fn absent_altered_hidden_and_unmatched_quote_sources_refuse() {
    let (profile, loaded, cell) = fixture();
    let missing = VerifiedDisclosureRegistry::verify(&profile, &[], [&cell]);
    assert_eq!(missing.refusals[0].refusal.code, "FMINE-REF-506");
    let mut altered = loaded.clone();
    altered.workbook.sha256[0] ^= 1;
    let digest = VerifiedDisclosureRegistry::verify(&profile, &[altered], [&cell]);
    assert_eq!(digest.refusals[0].refusal.code, "FMINE-REF-505");
    let mut hidden = loaded.clone();
    hidden.workbook.hidden_sheets = std::mem::take(&mut hidden.workbook.sheets);
    let hidden = VerifiedDisclosureRegistry::verify(&profile, &[hidden], [&cell]);
    assert_eq!(hidden.refusals[0].refusal.code, "FMINE-REF-208");
    let mut wrong_quote = profile.clone();
    wrong_quote.restatement_notes[0]
        .quoted_text
        .push_str(" More unprinted words.");
    let wrong =
        VerifiedDisclosureRegistry::verify(&wrong_quote, std::slice::from_ref(&loaded), [&cell]);
    assert!(!wrong.is_complete());
    assert!(wrong.matching(&cell.key, &cell.key).is_empty());
    assert_eq!(wrong.refusals[0].refusal.code, "FMINE-REF-208");
    let mut whitespace = profile.clone();
    whitespace.restatement_notes[0].quoted_text =
        "Comparative\n production for Site A has been restated. ".into();
    assert!(VerifiedDisclosureRegistry::verify(&whitespace, &[loaded], [&cell]).is_complete());
}

#[test]
fn scope_requires_observed_semantics_whole_period_and_unique_unit() {
    let (profile, loaded, cell) = fixture();
    let mut typo = profile.clone();
    typo.restatement_scopes[0].line.push_str("_typo");
    let wrong = VerifiedDisclosureRegistry::verify(&typo, std::slice::from_ref(&loaded), [&cell]);
    assert!(!wrong.is_complete());
    assert_eq!(wrong.refusals[0].refusal.code, "FMINE-REF-208");
    let mut other_unit = cell.clone();
    other_unit.key.unit = "tonne".into();
    assert!(
        !VerifiedDisclosureRegistry::verify(
            &profile,
            std::slice::from_ref(&loaded),
            [&cell, &other_unit]
        )
        .is_complete()
    );
    let mut year = cell.clone();
    year.key.valid_period.kind = PeriodKind::FullYear;
    let whole =
        VerifiedDisclosureRegistry::verify(&profile, std::slice::from_ref(&loaded), [&year]);
    assert_eq!(whole.matching(&year.key, &year.key).len(), 1);
    let mut partial = profile.clone();
    partial.restatement_scopes[0].period_to = "2030Q3".into();
    let part = VerifiedDisclosureRegistry::verify(&partial, &[loaded], [&year]);
    assert!(
        !part.is_complete(),
        "nine months do not cover a full-year assertion"
    );
}

#[test]
fn disclosure_digest_binds_scope_and_source_verification() {
    let (profile, loaded, cell) = fixture();
    let good = VerifiedDisclosureRegistry::verify(&profile, std::slice::from_ref(&loaded), [&cell]);
    let mut changed = profile.clone();
    changed.restatement_scopes[0].period_to = "2030Q3".into();
    let changed =
        VerifiedDisclosureRegistry::verify(&changed, std::slice::from_ref(&loaded), [&cell]);
    assert!(changed.is_complete());
    assert_ne!(good.digest(), changed.digest());
    let missing = VerifiedDisclosureRegistry::verify(&profile, &[], [&cell]);
    assert_ne!(good.digest(), missing.digest());
    let mut removed = profile;
    removed.restatement_notes.clear();
    removed.restatement_scopes.clear();
    let empty = VerifiedDisclosureRegistry::verify(&removed, &[loaded], [&cell]);
    assert!(empty.is_complete());
    assert_ne!(good.digest(), empty.digest());
    assert!(empty.matching(&cell.key, &cell.key).is_empty());
}
