//! DESIGN (BD-341/BD-342): independent calendar and declared workbook-availability controls.
//! Determinism: Exact (invented source declarations; no wall clock or network).

use fmine_registry::sources::{
    RegisteredSource, SourceKind, SourceRow, available_date, day_end, parse_sources_registry,
    publication_cut, publication_date, retrieval_upper,
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
        retrieved_utc: None,
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

fn registry(override_time: Option<&str>) -> String {
    format!(
        r#"schema = "fmine.sources.v1"
issuer = "invented"
corpus_dir = "invented"
retrieved_utc = "2030-07-02T10:00:00Z/10:01:00Z"
url_prefix = "https://example.invalid/"
[[source]]
file = "old.xlsx"
kind = "mine_stats"
quarter = "2030Q2"
path = "old.xlsx"
url_basis = "W1 table"
bytes = 1
sha256 = "{}"
[[source]]
file = "new.xlsx"
kind = "mine_stats"
quarter = "2030Q3"
path = "new.xlsx"
url_basis = "W1 table"
bytes = 1
sha256 = "{}"
{}
[[subject]]
name = "Invented site"
"#,
        "a".repeat(64),
        "b".repeat(64),
        override_time.map_or_else(String::new, |time| format!("retrieved_utc = \"{time}\""))
    )
}

#[test]
fn new_vintage_retrieval_does_not_move_old_source_availability() {
    let parsed =
        parse_sources_registry(&registry(Some("2030-10-02T23:59:59Z/2030-10-03T00:00:01Z")))
            .unwrap();
    assert_eq!(parsed.sources[0].retrieved_utc, None);
    let profile = fmine_public::sources::IssuerProfile::from_registry(parsed).unwrap();
    let old_cut = profile.mine_stats_as_of("2030-07-02").unwrap();
    assert!(old_cut.includes("old.xlsx"));
    assert!(!old_cut.includes("new.xlsx"));
    assert_eq!(old_cut.available_by.as_deref(), Some("2030-07-02"));
    assert!(
        !profile
            .mine_stats_as_of("2030-10-02")
            .unwrap()
            .includes("new.xlsx")
    );
    let new_cut = profile.mine_stats_as_of("2030-10-03").unwrap();
    assert!(new_cut.includes("new.xlsx"));
    assert_eq!(old_cut.sources[1], new_cut.sources[1]); // Sorted old source, untouched.
    assert_eq!(
        new_cut.sources[0].basis,
        "2030-10-02T23:59:59Z/2030-10-03T00:00:01Z"
    );
    let legacy = fmine_public::sources::IssuerProfile::from_registry(
        parse_sources_registry(&registry(None)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        legacy.mine_stats_as_of("2030-07-02").unwrap().sources[1],
        old_cut.sources[1]
    );
}

#[test]
fn malformed_per_source_retrieval_never_falls_back_to_the_batch() {
    assert!(parse_sources_registry(&registry(Some("2030-10-03"))).is_ok());
    for bad in [
        "",
        "unknown",
        "2030-02-29",
        "2030-10-03T01:00:00Z/00:59:59Z",
        "2030-10-03T00:00:00+00:00",
    ] {
        assert!(
            parse_sources_registry(&registry(Some(bad))).is_err(),
            "{bad}"
        );
    }
    let valid = registry(Some("2030-10-03"));
    let duplicate = valid.replace(
        "retrieved_utc = \"2030-10-03\"",
        "retrieved_utc = \"2030-10-03\"\nretrieved_utc = \"2030-10-04\"",
    );
    assert!(parse_sources_registry(&duplicate).is_err());
}
