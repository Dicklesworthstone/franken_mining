//! Exact public protocol contracts for synthetic quarterly forecasts; no issuer or timestamp evidence.

use fmine_calib::record::candidate::{CandidateSeries, build_candidate};
use fmine_calib::record::{
    BaselineKind, Outcome, Quarter, Rat, RecordRules, TargetMetric, parse_rules, parse_utc,
};

const ANNUAL: &str = include_str!("../../../forecasts/fy2026/rules.toml");

#[allow(clippy::unwrap_used, reason = "the synthetic rules fixture must parse")]
fn quarterly(point: &str) -> RecordRules {
    parse_rules(
        &ANNUAL
            .replace("fy_minus_9m", "quarter_qtd")
            .replace("q4_qtd", "quarter_qtd")
            .replace(
                "point_quarter = \"2026Q3\"",
                &format!("point_quarter = \"{point}\""),
            ),
    )
    .unwrap()
}

#[test]
fn quarterly_rules_bind_one_next_quarter_and_preserve_the_annual_contract() {
    let annual = parse_rules(ANNUAL).unwrap();
    assert_eq!(annual.outcome_quarter().unwrap().to_string(), "2026Q4");
    for (point, outcome, end) in [
        ("2026Q4", "2027Q1", "2027-04-01T00:00:00Z"),
        ("2027Q1", "2027Q2", "2027-07-01T00:00:00Z"),
        ("2027Q2", "2027Q3", "2027-10-01T00:00:00Z"),
        ("2027Q3", "2027Q4", "2028-01-01T00:00:00Z"),
    ] {
        let quarter = quarterly(point).outcome_quarter().unwrap();
        assert_eq!(quarter.to_string(), outcome);
        assert_eq!(quarter.end_utc(), parse_utc(end));
    }
    let base = quarterly("2026Q4");
    let mut rules = base.clone();
    rules.targets[1].outcome = Outcome::Q4Qtd;
    assert!(
        rules.outcome_quarter().is_err(),
        "annual and quarterly meanings cannot mix"
    );
    let index = base
        .baselines
        .iter()
        .position(|b| matches!(b.kind, BaselineKind::Persistence { .. }))
        .unwrap();
    for point in [
        Quarter { year: 2027, q: 1 },
        Quarter { year: 2030, q: 0 },
        Quarter { year: 0, q: 4 },
        Quarter {
            year: i32::MAX,
            q: 4,
        },
    ] {
        let mut rules = base.clone();
        if let BaselineKind::Persistence {
            point: ref mut value,
            ..
        } = rules.baselines[index].kind
        {
            *value = point;
        }
        assert!(
            rules.outcome_quarter().is_err(),
            "invalid or mismatched direct in-memory rules refuse"
        );
    }
    let mut rules = base.clone();
    rules.baselines.remove(index);
    assert!(
        rules.outcome_quarter().is_err(),
        "each target needs a point"
    );
    let mut rules = base.clone();
    rules.baselines.push(rules.baselines[index].clone());
    assert!(
        rules.outcome_quarter().is_err(),
        "duplicate pools cannot select a preferred point"
    );
    assert!(
        parse_rules(&ANNUAL.replace("2026Q3", "2026Q4"))
            .unwrap()
            .outcome_quarter()
            .is_err()
    );
    for invalid in [
        Quarter { year: 9999, q: 4 },
        Quarter {
            year: i32::MAX,
            q: 4,
        },
        Quarter { year: 2030, q: 0 },
    ] {
        assert_eq!(invalid.end_utc(), None);
    }
}

#[test]
fn quarter_qtd_candidates_keep_exact_bounds_units_and_signed_cost_domains() {
    let mut rules = quarterly("2026Q4");
    rules.targets.truncate(1);
    rules.baselines.retain(|b| b.target == rules.targets[0].id);
    rules.sites.truncate(1);
    rules.sites[0].name = "Synthetic Mine".into();
    for baseline in &mut rules.baselines {
        if matches!(baseline.kind, BaselineKind::Persistence { .. }) {
            baseline.kind = BaselineKind::Persistence {
                pool_from: Quarter { year: 2026, q: 1 },
                pool_through: Quarter { year: 2026, q: 3 },
                point: Quarter { year: 2026, q: 4 },
            };
        }
    }
    let mut series = vec![CandidateSeries {
        site: "Synthetic Mine".into(),
        target: rules.targets[0].id.clone(),
        history: [100, 110, 132].map(|n| Some(Rat::from_integer(n))).to_vec(),
        point: Some(Rat::from_integer(200)),
    }];
    for (field, unit) in [
        ("au_prod_100_koz", "koz"),
        ("cu_prod_100_kt", "kt"),
        ("aisc_usd_per_oz", "usd_per_oz"),
        ("c1_usd_per_lb", "usd_per_lb"),
        ("aisc_usd_per_lb", "usd_per_lb"),
    ] {
        rules.targets[0].field = field.into();
        let metric = TargetMetric::from_target(&rules.targets[0]).unwrap();
        assert_eq!(metric.unit(), unit);
        series[0].point = Some(Rat::from_integer(200));
        let candidate = build_candidate(&rules, &series, 2).unwrap();
        let row = &candidate.exact_rows[0];
        assert_eq!(
            (row.lo, row.point, row.hi),
            (
                Rat::from_integer(220),
                Rat::from_integer(220),
                Rat::from_integer(240)
            )
        );
        series[0].point = Some(Rat::from_integer(-200));
        let signed = build_candidate(&rules, &series, 2);
        if metric.is_production() {
            assert!(signed.is_err());
        } else {
            let signed = signed.unwrap();
            assert_eq!(
                (signed.exact_rows[0].lo, signed.exact_rows[0].hi),
                (Rat::from_integer(-240), Rat::from_integer(-220))
            );
        }
    }
}
