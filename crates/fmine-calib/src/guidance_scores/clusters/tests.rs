//! Known answers and invented panels; no issuer data or calibrated-forecast claim.
//! Determinism: Exact (keyed integer panels and exact rational assertions).

use super::*;

fn r(n: i128, d: i128) -> Rat {
    Rat::new(n, d).unwrap()
}

fn member(site: usize, year: u16, value: Rat) -> Member {
    Member {
        id: format!("site-{site}/{year}"),
        site: format!("site-{site}"),
        year,
        guidance: Some(Guidance::Range {
            lo: r(-1, 1),
            hi: r(1, 1),
        }),
        outcome: Some(PrintedOutcome {
            value,
            interval: closed(value, value),
        }),
    }
}

#[test]
fn exact_equal_cluster_estimand_differs_from_pooled_mean_and_retains_missing_members() {
    let mut rows = vec![
        member(0, 2030, r(1, 1)),
        member(1, 2030, r(3, 1)),
        member(0, 2031, r(10, 1)),
    ];
    let report = assess(&rows, Axis::Year).unwrap();
    assert_eq!(report.bias.point, Some(r(6, 1)));
    assert_eq!(report.bias.unavailable, Some("fewer_than_20_clusters"));
    assert_eq!(report.years[0].observed_mean_z, Some(r(2, 1)));
    assert_eq!(report.sites[0].observed_mean_z, Some(r(11, 2)));
    assert_eq!(report.hit.point, Some(r(1, 4)));
    let site = assess(&rows, Axis::Site).unwrap();
    assert_eq!(site.bias.point, Some(r(17, 4)));
    rows[1].outcome = None;
    let missing = assess(&rows, Axis::Year).unwrap();
    assert_eq!(missing.n_members, 3);
    assert_eq!(missing.n_unresolved, 1);
    assert_eq!(missing.years[0].members.len(), 2);
    assert_eq!(missing.years[0].observed_mean_z, Some(r(1, 1)));
    assert_eq!(
        missing.hit.completion_bounds,
        Some(closed(r(1, 4), r(1, 2)))
    );
    assert_eq!(missing.bias.point, None);
    assert_eq!(missing.bias.unavailable, Some("unresolved_members"));
    assert_eq!(missing.hit.unavailable, Some("unresolved_members"));
}

#[test]
fn percentile_known_answer_retains_separate_rounding_enclosure() {
    // Two equally probable blocks, with two draws: mean 0, 1, 2 with masses 1/4, 1/2, 1/4.
    // Every block's lower/upper bound shifts by one, so the enclosure is exactly [-1, 3].
    let values = [
        (r(0, 1), closed(r(-1, 1), r(1, 1))),
        (r(2, 1), closed(r(1, 1), r(3, 1))),
    ];
    let (point, rounding) = bootstrap(&values, Axis::Year, 1, REPLICATES).unwrap();
    assert_eq!(point, closed(r(0, 1), r(2, 1)));
    assert_eq!(rounding, closed(r(-1, 1), r(3, 1)));

    let mut row = member(0, 2030, r(0, 1));
    row.outcome.as_mut().unwrap().interval = Interval {
        lo: r(-1, 10),
        hi: r(1, 10),
        lo_strict: true,
        hi_strict: false,
    };
    let report = assess(&[row], Axis::Year).unwrap();
    let expected = Interval {
        lo: r(-1, 10),
        hi: r(1, 10),
        lo_strict: true,
        hi_strict: false,
    };
    assert_eq!(report.years[0].observed_mean_z_rounding, Some(expected));
    assert_eq!(report.bias.rounding_bounds, Some(expected));
}

#[test]
fn block_resampling_never_counts_replicated_members_as_new_independent_clusters() {
    let rows: Vec<_> = (2000..2040)
        .flat_map(|year| (0..2).map(move |site| member(site, year, r(i128::from(year) - 2020, 10))))
        .collect();
    let base = assess(&rows, Axis::Year).unwrap();
    assert!(base.bias.interval_90.is_some());
    let duplicate: Vec<_> = rows
        .iter()
        .flat_map(|row| {
            [
                row.clone(),
                Member {
                    id: format!("copy/{}", row.id),
                    site: format!("copy/{}", row.site),
                    ..row.clone()
                },
            ]
        })
        .collect();
    let copied = assess(&duplicate, Axis::Year).unwrap();
    assert_eq!(base.bias, copied.bias);
    assert_eq!(base.hit, copied.hit);
    assert_eq!(base.years.len(), copied.years.len());
    let mut reversed = rows.clone();
    reversed.reverse();
    assert_eq!(base, assess(&reversed, Axis::Year).unwrap());
    let scaled: Vec<_> = rows
        .iter()
        .map(|row| {
            let factor = r(311034768, 10000000); // exact oz-to-gram unit change
            let scale = |x: Rat| x.checked_mul(factor).unwrap();
            let o = row.outcome.unwrap();
            Member {
                guidance: Some(Guidance::Range {
                    lo: scale(r(-1, 1)),
                    hi: scale(r(1, 1)),
                }),
                outcome: Some(PrintedOutcome {
                    value: scale(o.value),
                    interval: closed(scale(o.interval.lo), scale(o.interval.hi)),
                }),
                ..row.clone()
            }
        })
        .collect();
    assert_eq!(base, assess(&scaled, Axis::Year).unwrap());
}

#[test]
fn sparse_degenerate_unknown_and_nonrange_populations_never_gain_false_intervals() {
    let mut rows: Vec<_> = (2000..2020).map(|year| member(0, year, r(0, 1))).collect();
    let constant = assess(&rows, Axis::Year).unwrap();
    assert_eq!(
        constant.hit.unavailable,
        Some("no_between_cluster_variation")
    );
    assert_eq!(
        constant.bias.unavailable,
        Some("no_between_cluster_variation")
    );
    let sparse = assess(&rows[..19], Axis::Year).unwrap();
    assert_eq!(sparse.bias.unavailable, Some("fewer_than_20_clusters"));
    assert_eq!(sparse.bias.point, Some(r(0, 1)));
    rows[0].guidance = Some(Guidance::AtMost { upper: r(1, 1) });
    let nonrange = assess(&rows, Axis::Year).unwrap();
    assert_eq!(
        nonrange.bias.unavailable,
        Some("cluster_without_range_guidance")
    );
    assert_eq!(nonrange.hit.point, Some(r(1, 1)));
    rows[0].guidance = None;
    rows[0].outcome = None;
    let unknown = assess(&rows, Axis::Year).unwrap();
    assert_eq!(unknown.years[0].unknown_guidance, 1);
    assert_eq!(
        unknown.hit.completion_bounds,
        Some(closed(r(19, 20), r(1, 1)))
    );
    assert_eq!(unknown.bias.unavailable, Some("unresolved_members"));
    let mut duplicate = rows.clone();
    duplicate.push(rows[0].clone());
    assert!(assess(&duplicate, Axis::Year).is_err());
    rows[0].outcome = rows[1].outcome;
    assert!(assess(&rows, Axis::Year).is_err());
    assert_eq!(
        assess(&[], Axis::Year).unwrap().hit.unavailable,
        Some("empty_population")
    );
}

#[test]
fn exact_bootstrap_overflow_refuses_instead_of_approximating_or_dropping_a_cluster() {
    let values = [
        (
            r(i128::MAX - 1, 1),
            closed(r(i128::MAX - 1, 1), r(i128::MAX - 1, 1)),
        ),
        (
            r(i128::MAX - 2, 1),
            closed(r(i128::MAX - 2, 1), r(i128::MAX - 2, 1)),
        ),
    ];
    assert_eq!(
        bootstrap(&values, Axis::Year, 1, REPLICATES)
            .unwrap_err()
            .code,
        "FMINE-REF-207"
    );
}

#[test]
fn synthetic_coverage_reports_correct_and_wrong_cluster_axes_over_200_panels() {
    // DESIGN: 40x40 balanced panels, mu=0.2, unit marginal variance. The two dominant
    // cases deliberately have only one nonzero shared component. The opposite axis is
    // misspecified: its false precision must remain visible rather than being advertised as robust.
    let truth_hit = fmine_core::det::norm_cdf(0.8) - fmine_core::det::norm_cdf(-1.2);
    let contains = |interval: Option<Interval>, value: f64| {
        interval.is_some_and(|i| i.lo.to_f64() <= value && value <= i.hi.to_f64())
    };
    for (scenario, year_sd, site_sd, residual_sd, axis) in [
        ("independent", 0.0, 0.0, 1.0, Axis::Year),
        (
            "year_dominant",
            fmine_core::det::sqrt(0.99),
            0.0,
            0.1,
            Axis::Year,
        ),
        (
            "site_dominant",
            0.0,
            fmine_core::det::sqrt(0.99),
            0.1,
            Axis::Site,
        ),
    ] {
        let (mut bias_cover, mut hit_cover, mut wrong_bias_cover, mut wrong_hit_cover) =
            (0, 0, 0, 0);
        for replication in 0..200 {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 0x0341_5ca1,
                partition: 341,
                domain: 99,
                var_group: 0,
                realization: replication,
                brick: 0,
            });
            let years: Vec<_> = (0..40).map(|_| year_sd * rng.next_normal()).collect();
            let sites: Vec<_> = (0..40).map(|_| site_sd * rng.next_normal()).collect();
            let mut rows = Vec::with_capacity(1600);
            for (y, effect_y) in years.iter().enumerate() {
                for (s, effect_s) in sites.iter().enumerate() {
                    let z = 0.2 + effect_y + effect_s + residual_sd * rng.next_normal();
                    rows.push(member(
                        s,
                        2000 + y as u16,
                        r((z * 1_000_000.0).round() as i128, 1_000_000),
                    ));
                }
            }
            let report = assess_with_replicates(&rows, axis, 399).unwrap();
            bias_cover += usize::from(contains(report.bias.interval_90, 0.2));
            hit_cover += usize::from(contains(report.hit.interval_90, truth_hit));
            if scenario != "independent" {
                let opposite = if axis == Axis::Year {
                    Axis::Site
                } else {
                    Axis::Year
                };
                let wrong = assess_with_replicates(&rows, opposite, 399).unwrap();
                wrong_bias_cover += usize::from(contains(wrong.bias.interval_90, 0.2));
                wrong_hit_cover += usize::from(contains(wrong.hit.interval_90, truth_hit));
            }
        }
        println!(
            "synthetic_guidance_clusters scenario={scenario} panels=200 sites=40 years=40 bootstrap_replicates=399 bias_cover={bias_cover}/200 hit_cover={hit_cover}/200 wrong_axis_bias_cover={wrong_bias_cover}/200 wrong_axis_hit_cover={wrong_hit_cover}/200"
        );
        assert!(
            (170..=190).contains(&bias_cover),
            "{scenario}: {bias_cover}/200 bias coverage"
        );
        assert!(
            (170..=190).contains(&hit_cover),
            "{scenario}: {hit_cover}/200 hit coverage"
        );
        if scenario != "independent" {
            assert!(
                wrong_bias_cover < 120 && wrong_hit_cover < 120,
                "misspecified axis must expose common-shock failure"
            );
        }
    }
}
