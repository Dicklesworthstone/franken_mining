//! DESIGN (BD-341/BD-342): independent calendar and declared workbook-availability controls.
//! Determinism: Exact (invented source declarations; no wall clock or network).

use fmine_registry::sources::{
    RegisteredSource, SourceKind, SourceRow, available_date, day_end, publication_cut,
    publication_date, retrieval_upper,
};

#[test]
fn retrieval_upper_preserves_strict_utc_and_monotone_interval_semantics() {
    assert_eq!(day_end("1970-01-01").unwrap(), 86_399);
    assert_eq!(retrieval_upper("1970-01-02T00:00:01Z").unwrap(), 86_401);
    assert_eq!(
        retrieval_upper("1970-01-01T00:00:00Z/01:00:02Z").unwrap(),
        3_602
    );
    assert_eq!(retrieval_upper("1970-01-01/1970-01-02").unwrap(), 172_799);
    assert_eq!(
        retrieval_upper("1970-01-01T00:00:00Z/1970-01-02T00:00:00Z").unwrap(),
        86_400
    );
    for text in [
        "",
        "00:00:00Z",
        "1970-01-01T24:00:00Z",
        "1970-01-01T00:60:00Z",
        "1970-01-01T00:00:60Z",
        "1970-01-01T00:00:00+00:00",
        "1970-01-02/1970-01-01",
        "1970-01-01T02:00:00Z/01:00:00Z",
        "1970-01-01/",
        "2030-02-29",
        "éééééT00:00:00Z",
    ] {
        assert!(retrieval_upper(text).is_err(), "{text}");
    }
    assert!(day_end("2000-02-29").is_ok());
    assert!(day_end("1900-02-29").is_err());
}

#[test]
fn publication_precedes_late_retrieval_and_future_publication_overrides_early_retrieval() {
    let rows = [
        (
            "original.xlsx",
            Some("2031-01-15"),
            Some("FACT: invented original release"),
        ),
        (
            "comparative.xlsx",
            Some("2032-03-01"),
            Some("INFERENCE: invented dated archive"),
        ),
        ("undated.xlsx", None, None),
    ];
    let cut = publication_cut(rows, "2032-02-02T00:00:00Z", "2032-02-01").unwrap();
    assert_eq!(cut.available_by.as_deref(), Some("2031-01-15"));
    assert!(cut.includes("original.xlsx"));
    assert!(!cut.includes("comparative.xlsx"));
    assert!(!cut.includes("undated.xlsx"));
    assert_eq!(cut.sources[0].basis_kind, "registered_publication");
    assert_eq!(cut.sources[1].basis, "FACT: invented original release");
    assert_eq!(cut.sources[2].basis_kind, "verified_retrieval_upper_bound");
    assert_eq!(cut.sources[2].basis, "2032-02-02T00:00:00Z");
    let future = publication_cut([rows[1]], "2030-01-01", "2032-02-01").unwrap();
    assert_eq!(future.available_by, None);
    assert!(!future.includes("comparative.xlsx"));
    let mut reversed = rows;
    reversed.reverse();
    assert_eq!(
        cut,
        publication_cut(reversed, "2032-02-02T00:00:00Z", "2032-02-01").unwrap()
    );
    assert_eq!(
        available_date(None, None, "2032-02-01T23:59:59Z").unwrap(),
        "2032-02-01"
    );
}

#[test]
fn incomplete_or_forged_date_declarations_refuse_without_a_fallback() {
    for (date, basis) in [
        (Some("2031-01-15"), None),
        (None, Some("FACT: invented release")),
        (Some("2031-02-29"), Some("FACT: invented release")),
        (Some("2031-01-15"), Some("FACT: ")),
        (Some("2031-01-15"), Some("FACT: invented\nrelease")),
    ] {
        assert!(publication_date(date, basis).is_err());
        assert!(available_date(date, basis, "2030-01-01").is_err());
    }
    assert!(
        publication_cut(
            [("same", None, None), ("same", None, None)],
            "2030-01-01",
            "2031-01-01"
        )
        .is_err()
    );
    assert!(publication_cut([], "", "2031-02-29").is_err());
    let empty = publication_cut([], "", "2031-01-01").unwrap();
    assert!(empty.sources.is_empty());
    assert!(empty.available_by.is_none());
}

#[test]
fn typed_registered_projection_keeps_the_original_publication_citation() {
    let source = RegisteredSource::try_from(SourceRow {
        file: "invented.xlsx".into(),
        kind: "mine_stats".into(),
        quarter: "2030Q4".into(),
        url: "https://example.invalid/invented.xlsx".into(),
        url_basis: "W1 table".into(),
        bytes: 1,
        sha256: "a".repeat(64),
        published_at: Some("2031-01-15".into()),
        published_at_basis: Some("FACT: invented release".into()),
    })
    .unwrap();
    assert_eq!(source.kind, SourceKind::MineStats);
    assert_eq!(source.published_at.as_deref(), Some("2031-01-15"));
    assert_eq!(
        source.published_at_basis.as_deref(),
        Some("FACT: invented release")
    );
}
