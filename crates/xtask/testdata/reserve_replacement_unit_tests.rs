use super::*;
use fmine_registry::metrics::CreditBasis;
use fmine_registry::reserve_replacement::{Aggregate, Headline, YearSelection};
use fmine_registry::transcriptions::Locator;

const BASIS: &str = "attributable_gold_ex_ad";
type Fixture = (
    ReplacementRegistry,
    MetricRegistry,
    Vec<Accepted>,
    Vec<DocumentRow>,
);

fn document() -> DocumentRow {
    DocumentRow {
        file: "Synthetic.pdf".into(),
        dir: "synthetic".into(),
        kind: "mda".into(),
        url: "https://example.test/synthetic.pdf".into(),
        url_basis: "synthetic".into(),
        published_at: "2033-02-01".into(),
        published_at_basis: "synthetic".into(),
        pages: 3,
        bytes: 42,
        sha256: "1".repeat(64),
    }
}

fn reading(quantity: &str, token: &str, period: &str, unit: &str) -> Accepted {
    Accepted {
        locator: Locator {
            doc: document().file,
            page: 2,
            table: "Synthetic reserves".into(),
            row: quantity.into(),
            col: period.into(),
        },
        doc_sha256: document().sha256,
        token: token.into(),
        value: parse_token(token).unwrap(),
        quantity: quantity.into(),
        unit: unit.into(),
        basis: BASIS.into(),
        period: period.into(),
        readers: vec!["reader_b".into(), "reader_a".into()],
    }
}

fn fixture(net: [&str; 3], depletion: [&str; 3]) -> Fixture {
    let mut values = Vec::new();
    let mut years = Vec::new();
    for (index, year) in [2030, 2031, 2032].into_iter().enumerate() {
        let net_change = format!("net_{year}");
        let dep = format!("depletion_{year}");
        values.push(reading(&net_change, net[index], &year.to_string(), "Moz"));
        values.push(reading(&dep, depletion[index], &year.to_string(), "Moz"));
        years.push(YearSelection {
            year,
            doc: document().file,
            net_change,
            depletion: dep,
        });
    }
    let definition = MetricDefinition {
        id: "reserve.gold@synthetic".into(),
        family: MetricFamily::ReserveReplacement,
        unit: "percent".into(),
        formula: "cumulative net change / cumulative depletion * 100".into(),
        source: "Synthetic definition".into(),
        evidence: "synthetic".into(),
        credit_basis: CreditBasis::ByProduct,
        extends: Vec::new(),
        supersedes: None,
    };
    let rule = ReserveReplacementRule {
        definition: definition.id.clone(),
        metal: ReserveMetal::Gold,
        window_years: 3,
        amount_unit: "Moz".into(),
        basis: BASIS.into(),
        excludes_acquisitions_divestments: true,
        geo_definition: None,
    };
    let series = Series {
        id: "gold".into(),
        definition: definition.id.clone(),
        start_year: 2030,
        end_year: 2032,
        unit: "Moz".into(),
        basis: BASIS.into(),
        depletion_sign: DepletionSign::Negative,
        years,
        aggregate: None,
        headline: None,
    };
    let mut metrics = MetricRegistry::default();
    metrics.definitions.push(definition);
    metrics.reserve_replacements.push(rule);
    (
        ReplacementRegistry {
            issuer: "synthetic".into(),
            series: vec![series],
        },
        metrics,
        values,
        vec![document()],
    )
}

fn report(f: &Fixture) -> ReplacementReport {
    assess(&f.0, &f.1, &f.2, &f.3)
}
fn computed(r: &ReplacementReport) -> &SeriesResult {
    match &r.series[0] {
        SeriesAssessment::Computed(r) => r,
        other => panic!("{other:?}"),
    }
}
fn refusal(r: &ReplacementReport) -> &SeriesRefusal {
    match &r.series[0] {
        SeriesAssessment::Refused(r) => r,
        other => panic!("{other:?}"),
    }
}
fn q(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

#[test]
fn ratio_of_cumulative_amounts_preserves_negative_net_changes() {
    let f = fixture(["2.00", "6.00", "-1.00"], ["-1.00", "-2.00", "-1.00"]);
    let r = report(&f);
    let result = computed(&r);
    assert_eq!(result.annual.net_change.printed, q(7, 1));
    assert_eq!(result.annual.depletion.printed, q(4, 1));
    assert_eq!(result.annual.percent.printed, q(175, 1));
    assert_ne!(result.annual.percent.printed, q(200 + 300 - 100, 3));
    assert_eq!(
        result.annual.percent.bounds,
        Bounds {
            lo: q(139700, 803),
            hi: q(140300, 797)
        }
    );
    assert_eq!(result.years[2].net_change.value.printed, q(-1, 1));
    assert_eq!(result.years[0].depletion.source.token, "-1.00");
    let negative = report(&fixture(
        ["-2.00", "-6.00", "1.00"],
        ["-1.00", "-2.00", "-1.00"],
    ));
    assert_eq!(computed(&negative).annual.percent.printed, q(-175, 1));
    assert_eq!(
        computed(&negative).annual.percent.bounds,
        Bounds {
            lo: q(-140300, 797),
            hi: q(-139700, 803)
        }
    );
}

fn with_aggregate(mut f: Fixture, net: &str, dep: &str) -> Fixture {
    f.0.series[0].aggregate = Some(Aggregate {
        doc: document().file,
        net_change: "total_net".into(),
        depletion: "total_depletion".into(),
        period: "2030-2032".into(),
    });
    f.2.push(reading("total_net", net, "2030-2032", "Moz"));
    f.2.push(reading("total_depletion", dep, "2030-2032", "Moz"));
    f
}

fn with_headline(mut f: Fixture, token: &str, kind: HeadlineKind) -> Fixture {
    f.0.series[0].headline = Some(Headline {
        doc: document().file,
        quantity: "headline".into(),
        period: "2030-2032".into(),
        kind,
    });
    f.2.push(reading("headline", token, "2030-2032", "percent"));
    f
}

#[test]
fn separate_rounded_totals_narrow_bounds_without_overwriting_annual_points() {
    let f = with_headline(
        with_aggregate(
            fixture(["1.2", "2.2", "3.2"], ["-1.0", "-1.0", "-1.0"]),
            "6.55",
            "-3.00",
        ),
        "218",
        HeadlineKind::Rounded,
    );
    let r = report(&f);
    let result = computed(&r);
    assert_eq!(result.annual.net_change.printed, q(33, 5));
    assert_eq!(result.annual.percent.printed, q(220, 1));
    let aggregate = result.aggregate.as_ref().unwrap();
    assert_eq!(aggregate.totals.net_change.printed, q(131, 20));
    assert_eq!(aggregate.totals.percent.printed, q(655, 3));
    assert_eq!(
        result.joint.net_change,
        Bounds {
            lo: q(1309, 200),
            hi: q(1311, 200)
        }
    );
    assert_eq!(
        result.joint.percent,
        Bounds {
            lo: q(130900, 601),
            hi: q(131100, 599)
        }
    );
    assert_eq!(
        result.headline.as_ref().unwrap().state,
        HeadlineState::RoundedIntervalCompatible
    );
    assert!(
        compare(result.joint.percent.lo, result.annual.percent.bounds.lo)
            .unwrap()
            .is_gt()
    );
    assert!(
        compare(result.joint.percent.hi, result.annual.percent.bounds.hi)
            .unwrap()
            .is_lt()
    );
    assert!(r.to_json().contains("\"comparison_interval\":{\"lo\""));
}

#[test]
fn contradictory_totals_refuse_with_all_source_evidence() {
    for (net, dep, cause) in [
        ("9.00", "-3.00", "net-change"),
        ("6.60", "-9.00", "depletion"),
    ] {
        let f = with_headline(
            with_aggregate(
                fixture(["1.2", "2.2", "3.2"], ["-1.0", "-1.0", "-1.0"]),
                net,
                dep,
            ),
            "220",
            HeadlineKind::Rounded,
        );
        let r = report(&f);
        let refused = refusal(&r);
        assert!(refused.reason.contains(cause));
        assert!(refused.reason.contains("disjoint"));
        assert_eq!(refused.evidence.len(), 9);
        assert_eq!(refused.documents, f.3);
    }
}

#[test]
fn approximate_language_gets_no_invented_tolerance() {
    let base = fixture(["2.00", "6.00", "-1.00"], ["-1.00", "-2.00", "-1.00"]);
    for (token, kind, expected) in [
        (
            "175",
            HeadlineKind::Rounded,
            HeadlineState::RoundedIntervalCompatible,
        ),
        ("200", HeadlineKind::Rounded, HeadlineState::Incompatible),
        (
            "999",
            HeadlineKind::Approximate,
            HeadlineState::ApproximateUnassessed,
        ),
    ] {
        let r = report(&with_headline(base.clone(), token, kind));
        let result = computed(&r);
        assert_eq!(result.headline.as_ref().unwrap().state, expected);
        assert_eq!(result.annual.percent.printed, q(175, 1));
        if kind == HeadlineKind::Approximate {
            assert!(r.to_json().contains("\"comparison_interval\":null"));
        }
    }
}

#[test]
fn population_and_metric_rules_cannot_shrink_or_change_the_basis() {
    let base = fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]);
    for case in 0..10 {
        let mut f = base.clone();
        match case {
            0 => {
                f.0.series[0].years.pop();
            }
            1 => f.0.series[0].years[1].year = 2030,
            2 => f.1.reserve_replacements[0].window_years = 5,
            3 => f.1.reserve_replacements[0].amount_unit = "koz".into(),
            4 => f.1.reserve_replacements[0].basis = "consolidated_gold_ex_ad".into(),
            5 => f.1.reserve_replacements[0].metal = ReserveMetal::GoldEquivalent,
            6 => f.1.reserve_replacements[0].excludes_acquisitions_divestments = false,
            7 => f.1.reserve_replacements.clear(),
            8 => {
                f.1.reserve_replacements
                    .push(f.1.reserve_replacements[0].clone())
            }
            9 => f.1.definitions[0].family = MetricFamily::AllInCost,
            _ => unreachable!(),
        }
        assert!(!refusal(&report(&f)).reason.is_empty(), "case {case}");
    }
}

#[test]
fn source_mutations_are_not_admitted_as_agreed_readings() {
    let base = fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]);
    for case in 0..12 {
        let mut f = base.clone();
        match case {
            0 => f.2[0].token = "999".into(),
            1 => f.2[0].doc_sha256 = "2".repeat(64),
            2 => f.2[0].period = "2031".into(),
            3 => f.2[0].unit = "koz".into(),
            4 => f.2[0].basis = "attributable_geo_ex_ad".into(),
            5 => f.2[0].readers = vec!["same_reader".into(), "same_reader".into()],
            6 => f.2[0].locator.page = 0,
            7 => f.3.push(f.3[0].clone()),
            8 => f.2.push(f.2[0].clone()),
            9 => f.3[0].published_at = "2029-12-31".into(),
            10 => {
                f.2[0].token = "1 - 2".into();
                f.2[0].value = parse_token("1 - 2").unwrap();
            }
            11 => {
                f.2.remove(0);
            }
            _ => unreachable!(),
        }
        assert!(!refusal(&report(&f)).reason.is_empty(), "case {case}");
    }
}

#[test]
fn sign_convention_is_explicit_and_zero_crossing_denominator_refuses() {
    let negative = report(&fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]));
    let mut positive = fixture(["2.0", "6.0", "-1.0"], ["1.0", "2.0", "1.0"]);
    assert!(refusal(&report(&positive)).reason.contains("opposite sign"));
    positive.0.series[0].depletion_sign = DepletionSign::Positive;
    assert_eq!(
        computed(&negative).annual,
        computed(&report(&positive)).annual
    );
    let zero = fixture(["2", "6", "-1"], ["0", "0", "0"]);
    assert!(refusal(&report(&zero)).reason.contains("include zero"));
    let crossing = fixture(["2", "6", "-1"], ["-1", "0", "0"]);
    assert!(refusal(&report(&crossing)).reason.contains("include zero"));
}

#[test]
fn coherent_unit_and_precision_rescaling_preserves_the_full_ratio() {
    let moz = fixture(["1.234", "2.345", "3.456"], ["-0.111", "-0.222", "-0.333"]);
    let mut koz = fixture(["1234", "2345", "3456"], ["-111", "-222", "-333"]);
    koz.0.series[0].unit = "koz".into();
    koz.1.reserve_replacements[0].amount_unit = "koz".into();
    for a in &mut koz.2 {
        a.unit = "koz".into();
    }
    let a = report(&moz);
    let b = report(&koz);
    assert_eq!(computed(&a).annual.percent, computed(&b).annual.percent);
    assert_eq!(computed(&a).annual.percent.printed, q(117250, 111));
}

fn geo(mut f: Fixture) -> Fixture {
    let id = "geo.deck@synthetic".to_string();
    let mut definition = f.1.definitions[0].clone();
    definition.id = id.clone();
    definition.family = MetricFamily::Geo;
    definition.unit = "oz".into();
    f.1.definitions.push(definition);
    let rule = &mut f.1.reserve_replacements[0];
    rule.metal = ReserveMetal::GoldEquivalent;
    rule.geo_definition = Some(id.clone());
    rule.basis = "attributable_geo_ex_ad".into();
    f.0.series[0].basis = rule.basis.clone();
    for a in &mut f.2 {
        a.basis = rule.basis.clone();
    }
    for (year, gold, copper) in [(2030, "100", "1"), (2031, "200", "3"), (2032, "400", "6")] {
        f.1.geo_decks.push(GeoDeck {
            definition: id.clone(),
            year,
            gold_usd_per_oz: gold.into(),
            copper_usd_per_lb: copper.into(),
        });
    }
    f
}

#[test]
fn geo_retains_each_named_deck_without_reconverting_reported_values() {
    let mut f = geo(fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]));
    let first = report(&f);
    assert_eq!(computed(&first).annual.percent.printed, q(175, 1));
    assert_eq!(
        computed(&first)
            .years
            .iter()
            .map(|y| y.geo_deck.as_ref().unwrap().year)
            .collect::<Vec<_>>(),
        [2030, 2031, 2032]
    );
    f.1.geo_decks[0].gold_usd_per_oz = "999".into();
    let second = report(&f);
    assert_eq!(computed(&first).annual, computed(&second).annual);
    assert_ne!(first.digest(), second.digest());
    f.1.geo_decks.pop();
    assert!(refusal(&report(&f)).reason.contains("exactly one named"));
    f.1.geo_decks.push(f.1.geo_decks[0].clone());
    assert!(refusal(&report(&f)).reason.contains("exactly one named"));
}

#[test]
fn input_order_is_irrelevant_and_changed_source_bytes_change_the_report_digest() {
    let mut f = geo(fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]));
    let original = report(&f);
    f.0.series[0].years.reverse();
    f.1.definitions.reverse();
    f.1.geo_decks.reverse();
    f.2.reverse();
    for a in &mut f.2 {
        a.readers.reverse();
    }
    assert_eq!(original.to_json(), report(&f).to_json());
    f.3[0].sha256 = "2".repeat(64);
    assert!(refusal(&report(&f)).reason.contains("digest"));
    for a in &mut f.2 {
        a.doc_sha256 = f.3[0].sha256.clone();
    }
    let changed = report(&f);
    assert_eq!(computed(&original).annual, computed(&changed).annual);
    assert_ne!(original.digest(), changed.digest());
}

#[test]
fn all_synthetic_rounding_corners_are_enclosed_and_overflow_refuses() {
    let r = report(&fixture(["2.0", "6.0", "-1.0"], ["-1.0", "-2.0", "-1.0"]));
    let enclosure = computed(&r).annual.percent.bounds;
    // Independent integer twentieths, including a negative net-change operand.
    for mask in 0..64 {
        let offset = |bit| if mask & (1 << bit) == 0 { -1 } else { 1 };
        let numerator = 40 + offset(0) + 120 + offset(1) - 20 + offset(2);
        let denominator = 20 + offset(3) + 40 + offset(4) + 20 + offset(5);
        let percent = q(100 * numerator, denominator);
        assert!(!compare(percent, enclosure.lo).unwrap().is_lt());
        assert!(!compare(percent, enclosure.hi).unwrap().is_gt());
    }
    let overflow = report(&fixture(
        ["170141183460469231731687303715884105727", "1", "1"],
        ["-1", "-1", "-1"],
    ));
    assert!(refusal(&overflow).reason.contains("overflow"));
}
