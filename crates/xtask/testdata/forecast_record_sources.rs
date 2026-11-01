use super::*;
use fmine_calib::record::{Contact, Label};
use fmine_io::xlsx::{MinimalCell, MinimalSheet, Workbook, build_minimal_xlsx};
use fmine_public::key::{PublicValue, StatusKind, SubjectStatus};
use fmine_public::sources::RegisteredSource;
use fmine_registry::transcriptions::Locator;
use std::sync::atomic::{AtomicUsize, Ordering};

fn target(production: bool) -> Target {
    Target {
        id: if production {
            "production"
        } else {
            "unit_cost"
        }
        .into(),
        quantity: "invented public quantity".into(),
        scored_quantity: "invented frozen target".into(),
        field: if production {
            "au_prod_100_koz"
        } else {
            "aisc_usd_per_oz"
        }
        .into(),
        outcome: if production {
            Outcome::FyMinus9m
        } else {
            Outcome::Q4Qtd
        },
    }
}

fn observation(
    release: &str,
    period: Period,
    target: &Target,
    token: &str,
    disposed: bool,
) -> Observation {
    let (sheet, months, date) = match period.kind {
        PeriodKind::Quarter(1) => ("QTD", "three", "March 31"),
        PeriodKind::Quarter(2) => ("QTD", "three", "June 30"),
        PeriodKind::Quarter(3) => ("QTD", "three", "September 30"),
        PeriodKind::Quarter(_) => ("QTD", "three", "December 31"),
        PeriodKind::NineMonths => ("YTD", "nine", "September 30"),
        PeriodKind::FullYear => ("YTD", "twelve", "December 31"),
        PeriodKind::H1 => ("YTD", "six", "June 30"),
    };
    let file = format!("invented-{release}-{}-{}.xlsx", period.code(), target.id);
    let bytes = build_minimal_xlsx(&[MinimalSheet {
        name: sheet.into(),
        hidden: false,
        cells: vec![
            (
                "A3".into(),
                MinimalCell::Text(format!("For the {months} months ended {date},")),
            ),
            ("B3".into(), MinimalCell::Number(f64::from(period.year))),
            (
                "B4".into(),
                MinimalCell::Text(
                    if target.outcome == Outcome::FyMinus9m {
                        "Gold produced (000s oz 100%)"
                    } else {
                        "All-in sustaining costs ($/oz)"
                    }
                    .into(),
                ),
            ),
            ("A5".into(), MinimalCell::Text("Site A (100%)".into())),
            (
                "B5".into(),
                token
                    .parse::<f64>()
                    .map_or_else(|_| MinimalCell::Text(token.into()), MinimalCell::Number),
            ),
        ],
    }]);
    let workbook = Workbook::from_bytes(&file, &bytes).expect("synthetic workbook");
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.sources.clear();
    if disposed {
        profile.subject_statuses.push(SubjectStatus {
            subject: "Site A".into(),
            kind: StatusKind::Disposed,
            first: (2029, 1),
            last: None,
            evidence: "synthetic disposal".into(),
        });
    }
    let raw = read_mine_stats(&workbook, release, None)
        .expect("read invented cells")
        .values
        .into_iter()
        .next()
        .expect("one observation");
    let keyed =
        key::from_mine_stats(&profile, &workbook, release, &raw).map_err(|e| e.to_string());
    assert!(keyed.is_ok(), "{keyed:?}");
    Observation {
        release: Quarter::parse(release).unwrap(),
        raw,
        keyed,
        printed: Rat::from_decimal_str(token).map_err(|e| e.to_string()),
        file,
        hash: sha256(&bytes).to_hex(),
    }
}

fn inventory(observations: Vec<Observation>) -> Inventory {
    Inventory {
        releases: observations
            .iter()
            .map(|o| {
                (
                    o.release,
                    Release {
                        benchmark_reason: None,
                        outcome_reason: None,
                        unreadable: None,
                    },
                )
            })
            .collect(),
        observations,
        evidence: Evidence::default(),
    }
}

#[test]
fn original_release_selection_never_substitutes_comparatives_or_subtracts_rates() {
    let production = target(true);
    let cost = target(false);
    let fy = Period {
        year: 2030,
        kind: PeriodKind::FullYear,
    };
    let nine = Period {
        year: 2030,
        kind: PeriodKind::NineMonths,
    };
    let q4 = Period {
        year: 2030,
        kind: PeriodKind::Quarter(4),
    };
    let mut sources = inventory(vec![
        observation("2030Q4", fy, &production, "252", false),
        observation("2030Q3", nine, &production, "200", false),
        observation("2031Q4", fy, &production, "999", false),
        observation("2030Q4", q4, &cost, "120", false),
        observation("2030Q4", fy, &cost, "900", false),
        observation("2030Q3", nine, &cost, "800", false),
    ]);
    let outcome = infer_outcome(&mut sources, "Site A", &production, 2030).unwrap();
    assert_eq!(number(&outcome.value), Some(Rat::from_integer(52)));
    assert_eq!(outcome.locators.len(), 2);
    assert_eq!(
        number(
            &infer_outcome(&mut sources, "Site A", &cost, 2030)
                .unwrap()
                .value
        ),
        Some(Rat::from_integer(120))
    );
    sources
        .observations
        .retain(|o| !(o.release.to_string() == "2030Q4" && o.raw.label == production.field));
    assert!(matches!(
        infer_outcome(&mut sources, "Site A", &production, 2030)
            .unwrap()
            .value,
        Availability::Unavailable(_)
    ));
    sources
        .observations
        .push(observation("2030Q4", fy, &production, "190", false));
    let signed = infer_outcome(&mut sources, "Site A", &production, 2030).unwrap();
    assert_eq!(number(&signed.value), Some(Rat::from_integer(-10)));
    assert_eq!(signed.locators.len(), 2);
    assert!(
        sources
            .evidence
            .bindings
            .iter()
            .any(|binding| binding.contains("signed_original_cumulative_residual"))
    );
    assert!(
        sources
            .evidence
            .notes
            .iter()
            .any(|note| note.contains("not negative physical production"))
    );
    for (annual, nine_months, expected) in [("-10", "200", -210), ("190", "-200", 390)] {
        sources.observations = vec![
            observation("2030Q4", fy, &production, annual, false),
            observation("2030Q3", nine, &production, nine_months, false),
        ];
        let signed = infer_outcome(&mut sources, "Site A", &production, 2030).unwrap();
        assert_eq!(number(&signed.value), Some(Rat::from_integer(expected)));
        assert!(
            sources
                .evidence
                .bindings
                .iter()
                .any(|b| b.contains("reported_signed_production"))
        );
    }
}

#[test]
fn numeric_zero_survives_status_nil_while_dash_remains_unresolved() {
    let cost = target(false);
    let q4 = Period {
        year: 2030,
        kind: PeriodKind::Quarter(4),
    };
    let zero = observation("2030Q4", q4, &cost, "0", true);
    assert!(matches!(
        zero.keyed.as_ref().unwrap().claim.value,
        PublicValue::Nil { .. }
    ));
    let mut sources = inventory(vec![zero]);
    assert_eq!(
        number(
            &infer_outcome(&mut sources, "Site A", &cost, 2030)
                .unwrap()
                .value
        ),
        Some(Rat::from_integer(0))
    );
    assert!(
        sources
            .evidence
            .cells
            .values()
            .any(|s| s.contains("\"raw_kind\":\"number\",\"raw\":\"0\""))
    );
    sources.observations = vec![observation("2030Q4", q4, &cost, "—", true)];
    assert!(matches!(
        infer_outcome(&mut sources, "Site A", &cost, 2030)
            .unwrap()
            .value,
        Availability::Unavailable(_)
    ));
    sources.observations = vec![observation("2030Q4", q4, &cost, "-5", false)];
    assert_eq!(
        number(
            &infer_outcome(&mut sources, "Site A", &cost, 2030)
                .unwrap()
                .value
        ),
        Some(Rat::from_integer(-5)),
        "cost credits remain signed"
    );
}

fn manifest(inputs: Vec<InputDoc>) -> RecordManifest {
    RecordManifest {
        record_id: "synthetic-record".into(),
        author_role: "synthetic-author".into(),
        information_cutoff_utc: parse_utc("2030-10-15T00:00:00Z").unwrap(),
        label: Label::Nowcast,
        contact: Contact::PreContact,
        attestation: record::ATTESTATION.into(),
        rules_sha256: [0; 32],
        inputs,
    }
}

fn input(file: &str, bytes: &[u8], published: &str) -> InputDoc {
    InputDoc {
        doc_id: file.into(),
        url: format!("https://example.invalid/{file}"),
        publication_day: parse_date(published).unwrap(),
        retrieved_utc: parse_utc(&format!("{published}T12:00:00Z")).unwrap(),
        bytes: bytes.len() as u64,
        sha256: sha256(bytes).0,
        line: 1,
    }
}

#[test]
fn manifest_checks_reject_unused_hash_mismatch_and_premature_quarter_actuals() {
    let bytes = b"synthetic registered source";
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.sources = vec![RegisteredSource {
        file_name: "q3.xlsx".into(),
        kind: SourceKind::MineStats,
        quarter: "2030Q3".into(),
        url: "https://example.invalid/q3.xlsx".into(),
        url_basis: "invented".into(),
        bytes: bytes.len() as u64,
        sha256_hex: sha256(bytes).to_hex(),
    }];
    let mut rules =
        record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml")).unwrap();
    rules.issuer = "synthetic".into();
    let mut frozen = manifest(vec![input("q3.xlsx", bytes, "2030-10-01")]);
    profile.retrieved_utc = "2031-02-01T00:00:00Z".into();
    assert!(check_manifest_registry(&profile, &frozen, &rules).is_ok());
    assert_eq!(
        frozen_reason(&frozen, "q3.xlsx"),
        None,
        "later registry retrieval does not overwrite frozen input availability"
    );
    frozen.inputs[0].sha256[0] ^= 1;
    assert_eq!(
        check_manifest_registry(&profile, &frozen, &rules)
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
    frozen.inputs[0] = input("q3.xlsx", bytes, "2030-06-30");
    assert_eq!(
        check_manifest_registry(&profile, &frozen, &rules)
            .unwrap_err()
            .code,
        "FMINE-REF-108"
    );
    assert_eq!(
        retrieval_upper("2030-10-01T12:00:00Z/12:01:00Z").unwrap(),
        parse_utc("2030-10-01T12:01:00Z").unwrap()
    );
    assert!(retrieval_upper("2030-10-01T12:00:00Z/11:59:59Z").is_err());
}

#[test]
fn persistence_point_zero_and_signed_values_preserve_ordered_intervals() {
    for production in [false, true] {
        let cost = target(production);
        let mut rules =
            record::parse_rules(include_str!("../../../forecasts/fy2026/rules.toml"))
                .unwrap();
        rules.sites = vec![record::RulesSite {
            name: "Site A".into(),
            status: SiteStatus::Live,
            locator: "synthetic:QTD!A5".into(),
            reason: None,
        }];
        let rule = BaselineRule {
            id: "run_rate_persistence".into(),
            target: cost.id.clone(),
            kind: BaselineKind::Persistence {
                pool_from: Quarter::parse("2030Q1").unwrap(),
                pool_through: Quarter::parse("2030Q3").unwrap(),
                point: Quarter::parse("2031Q3").unwrap(),
            },
            rule: "fixed positive training pairs; multiply by printed point".into(),
            source: "synthetic original releases".into(),
            line: 1,
        };
        let point_period = Period {
            year: 2031,
            kind: PeriodKind::Quarter(3),
        };
        let mut sources = inventory(vec![
            observation(
                "2030Q1",
                Period {
                    year: 2030,
                    kind: PeriodKind::Quarter(1),
                },
                &cost,
                "100",
                false,
            ),
            observation(
                "2030Q2",
                Period {
                    year: 2030,
                    kind: PeriodKind::Quarter(2),
                },
                &cost,
                "50",
                false,
            ),
            observation(
                "2030Q3",
                Period {
                    year: 2030,
                    kind: PeriodKind::Quarter(3),
                },
                &cost,
                "100",
                false,
            ),
            observation("2031Q3", point_period, &cost, "0", false),
        ]);
        let zero = persistence(&mut sources, &rules, &cost, &rule).unwrap();
        assert_eq!(
            zero["Site A"],
            Availability::Available(Bounds {
                lo: Rat::from_integer(0),
                hi: Rat::from_integer(0)
            })
        );
        sources.observations[3] = observation("2031Q3", point_period, &cost, "-10", false);
        let signed = persistence(&mut sources, &rules, &cost, &rule).unwrap();
        assert_eq!(
            signed["Site A"],
            Availability::Available(Bounds {
                lo: Rat::from_integer(-20),
                hi: Rat::from_integer(-5)
            })
        );
        sources.observations[3] = observation("2031Q3", point_period, &cost, "—", false);
        let missing = persistence(&mut sources, &rules, &cost, &rule).unwrap();
        assert!(matches!(missing["Site A"], Availability::Unavailable(_)));
        assert!(
            sources
                .evidence
                .pools
                .iter()
                .all(|pool| pool.contains("\"ratios\":2,\"omitted_pairs\":0"))
        );
        let middle = Period {
            year: 2030,
            kind: PeriodKind::Quarter(2),
        };
        sources.observations[1] = observation("2030Q2", middle, &cost, "-50", false);
        sources.observations[3] = observation("2031Q3", point_period, &cost, "-10", false);
        let no_pool = persistence(&mut sources, &rules, &cost, &rule).unwrap();
        assert!(matches!(no_pool["Site A"], Availability::Unavailable(_)));
        let pool = sources.evidence.pools.last().unwrap();
        assert!(pool.contains("\"ratios\":0,\"omitted_pairs\":2"));
        assert!(pool.contains("printed value is not positive"));
    }
}

#[test]
fn same_day_guidance_requires_strictly_earlier_matching_manifest_retrieval() {
    let bytes = b"SYNTHETIC guidance";
    let mut document = DocumentRow {
        file: "guidance.pdf".into(),
        dir: ".".into(),
        kind: "mda".into(),
        url: "https://example.invalid/guidance.pdf".into(),
        url_basis: "invented".into(),
        published_at: "2030-10-01".into(),
        published_at_basis: "FACT: synthetic publication".into(),
        pages: 1,
        bytes: bytes.len() as u64,
        sha256: sha256(bytes).to_hex(),
    };
    let mut frozen = manifest(Vec::new());
    frozen.information_cutoff_utc = parse_utc("2030-10-15T12:00:00Z").unwrap();
    assert_eq!(
        guidance_timing(&document, &frozen).unwrap(),
        "registered_publication_date_before_cutoff_day"
    );
    document.published_at = "2030-10-15".into();
    assert!(guidance_timing(&document, &frozen).is_err());
    frozen
        .inputs
        .push(input(&document.file, bytes, &document.published_at));
    assert!(
        guidance_timing(&document, &frozen).is_err(),
        "retrieval exactly at the cutoff is not before it"
    );
    frozen.inputs[0].retrieved_utc -= 1;
    assert_eq!(
        guidance_timing(&document, &frozen).unwrap(),
        "manifest_declared_retrieval_before_cutoff"
    );
    frozen.inputs[0].sha256[0] ^= 1;
    assert!(guidance_timing(&document, &frozen).is_err());
    frozen.inputs[0].sha256[0] ^= 1;
    frozen.inputs[0].publication_day -= 1;
    assert!(guidance_timing(&document, &frozen).is_err());
    document.published_at = "2030-10-16".into();
    assert!(guidance_timing(&document, &frozen).is_err());
}

#[test]
fn latest_guidance_requires_agreement_and_converts_its_own_share_before_subtracting() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "fmine-record-guidance-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let bytes = b"SYNTHETIC invented guidance page, not issuer evidence";
    let mut profile = fmine_public::synthetic::synthetic_profile();
    profile.documents = [("older.pdf", "2030-01-01"), ("latest.pdf", "2030-10-01")]
        .into_iter()
        .map(|(file, published)| {
            std::fs::write(root.join(file), bytes).unwrap();
            DocumentRow {
                file: file.into(),
                dir: ".".into(),
                kind: "mda".into(),
                url: format!("https://example.invalid/{file}"),
                url_basis: "invented".into(),
                published_at: published.into(),
                published_at_basis: "invented".into(),
                pages: 1,
                bytes: bytes.len() as u64,
                sha256: sha256(bytes).to_hex(),
            }
        })
        .collect();
    // The latest earlier-day guidance is not required to be a forecast-generation input.
    let frozen = manifest(vec![input(
        "unrelated-input.txt",
        b"synthetic input",
        "2030-01-01",
    )]);
    let entry = |file: &str, reader: &str, token: &str| EntryRow {
        locator: Locator {
            doc: file.into(),
            page: 1,
            table: "Invented guidance".into(),
            row: "Site A".into(),
            col: "production".into(),
        },
        doc_sha256: sha256(bytes).to_hex(),
        token: token.into(),
        value: parse_token(token).unwrap(),
        quantity: "guidance_site_a_production_koz".into(),
        unit: "koz".into(),
        basis: "attributable:4/5".into(),
        period: "2030FY".into(),
        reader: reader.into(),
        line: 1,
    };
    let mut guidance = Guidance {
        registry: Some(TranscriptionRegistry {
            issuer: profile.issuer.clone(),
            entries: vec![
                entry("older.pdf", "one", "160 - 168"),
                entry("older.pdf", "two", "160 - 168"),
                entry("latest.pdf", "one", "200 - 208"),
            ],
        }),
        checked: BTreeMap::new(),
    };
    let production = target(true);
    let mut sources = inventory(vec![observation(
        "2030Q3",
        Period {
            year: 2030,
            kind: PeriodKind::NineMonths,
        },
        &production,
        "200",
        false,
    )]);
    assert!(
        matches!(
            guidance
                .resolve(
                    &root,
                    &profile,
                    &frozen,
                    &mut sources,
                    "Site A",
                    &production,
                    2030
                )
                .unwrap(),
            Availability::Unavailable(_)
        ),
        "pending newest guidance cannot fall back to older complete guidance"
    );
    guidance
        .registry
        .as_mut()
        .unwrap()
        .entries
        .push(entry("latest.pdf", "two", "200 - 208"));
    let resolved = guidance
        .resolve(
            &root,
            &profile,
            &frozen,
            &mut sources,
            "Site A",
            &production,
            2030,
        )
        .unwrap();
    assert_eq!(
        resolved,
        Availability::Available(Bounds {
            lo: Rat::from_integer(50),
            hi: Rat::from_integer(60)
        })
    );
    assert!(
        sources.evidence.documents["latest.pdf"].contains("\"in_forecast_manifest\":false")
    );
    for (token, low, high) in [("80 - 88", -100, -90), ("-8 - 0", -210, -200)] {
        for row in guidance
            .registry
            .as_mut()
            .unwrap()
            .entries
            .iter_mut()
            .filter(|e| e.locator.doc == "latest.pdf")
        {
            row.token = token.into();
            row.value = parse_token(&row.token).unwrap();
        }
        assert_eq!(
            guidance
                .resolve(
                    &root,
                    &profile,
                    &frozen,
                    &mut sources,
                    "Site A",
                    &production,
                    2030
                )
                .unwrap(),
            Availability::Available(Bounds {
                lo: Rat::from_integer(low),
                hi: Rat::from_integer(high)
            }),
            "signed annual guidance and implied Q4 baselines are retained, never clamped"
        );
    }
    std::fs::write(root.join("latest.pdf"), b"altered synthetic page").unwrap();
    guidance.checked.clear();
    assert_eq!(
        guidance
            .resolve(
                &root,
                &profile,
                &frozen,
                &mut sources,
                "Site A",
                &production,
                2030
            )
            .unwrap_err()
            .code,
        "FMINE-REF-505"
    );
}
