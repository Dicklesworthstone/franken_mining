#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_inputs_keep_exact_layer_and_arithmetic_limits_never_become_numbers() {
        use super::cost_layers::amounts;
        let exact = |value| CostLayerOperand {
            at: Some("synthetic-exact-input".into()),
            value: CostLayerValue::Number {
                value: Rat::from_integer(value),
                step: Rat::from_integer(0),
            },
        };
        let regions = [
            CostLayerRegion {
                region: "A".into(),
                mapped: true,
                rate: exact(1000),
                sold: exact(100),
            },
            CostLayerRegion {
                region: "B".into(),
                mapped: true,
                rate: exact(2000),
                sold: exact(300),
            },
        ];
        let result = amounts(&exact(1800), &regions).unwrap();
        let fifty = Rat::from_integer(50);
        assert_eq!(result.point, fifty);
        assert_eq!(result.interval, (fifty, fifty));
        assert_eq!(
            result.weighted_interval,
            (Rat::from_integer(1750), Rat::from_integer(1750))
        );
        assert_eq!(
            amounts(&exact(1800), &vec![regions[0].clone(); 1024])
                .unwrap()
                .point,
            Rat::from_integer(800)
        );
        let overflow = [CostLayerRegion {
            region: "A".into(),
            mapped: true,
            rate: exact(i128::MAX),
            sold: exact(2),
        }];
        assert_eq!(
            amounts(&exact(1800), &overflow).unwrap_err().reason,
            "exact_arithmetic_unavailable"
        );
    }

    #[test]
    fn company_layer_sweep_bounds_match_independent_exhaustive_small_boxes() {
        use super::cost_layers::weighted_mean_bounds;
        let mut seed = 719_u64;
        let mut next = |n: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        for _ in 0..100 {
            let parts: Vec<(Interval, Interval)> = (0..1 + next(6))
                .map(|_| {
                    let rate = i128::from(next(1000)) - 500;
                    let weight = i128::from(next(20)) + 1;
                    (
                        (
                            Rat::new(rate, 10).unwrap(),
                            Rat::new(rate + i128::from(next(20)), 10).unwrap(),
                        ),
                        (
                            Rat::new(weight, 10).unwrap(),
                            Rat::new(weight + i128::from(next(20)), 10).unwrap(),
                        ),
                    )
                })
                .collect();
            let mut reference = None;
            for mask in 0..1_u64 << parts.len() {
                let (mut lower, mut upper, mut total) = (
                    Rat::from_integer(0),
                    Rat::from_integer(0),
                    Rat::from_integer(0),
                );
                for (k, &((a, b), (c, d))) in parts.iter().enumerate() {
                    let w = if mask & (1 << k) == 0 { c } else { d };
                    total = total.checked_add(w).unwrap();
                    lower = lower.checked_add(a.checked_mul(w).unwrap()).unwrap();
                    upper = upper.checked_add(b.checked_mul(w).unwrap()).unwrap();
                }
                let (low, high) = (
                    lower.checked_div(total).unwrap(),
                    upper.checked_div(total).unwrap(),
                );
                reference = Some(match reference {
                    None => (low, high),
                    Some((old_low, old_high)) => (
                        if low.checked_cmp(old_low).unwrap().is_lt() {
                            low
                        } else {
                            old_low
                        },
                        if high.checked_cmp(old_high).unwrap().is_gt() {
                            high
                        } else {
                            old_high
                        },
                    ),
                });
            }
            assert_eq!(weighted_mean_bounds(&parts), reference);
        }
    }

    /// [`sum_holds`] against the kernel posing Σ lines = total with every box rounded once, on pseudo-random
    /// values of mixed steps around the slack bound.
    #[test]
    fn test_closed_form_sum_matches_the_kernel() {
        let period = Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        };
        let steps = [
            Rat::from_integer(1),
            Rat::new(1, 10).expect("step"),
            Rat::new(1, 100).expect("step"),
        ];
        let mut seed = 0x5eed_u64;
        let mut next = |n: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        let slot = |locator: String, units: i128, step: Rat| Slot {
            locator,
            operand: Operand::Number {
                value: Rat::from_integer(units).checked_mul(step).expect("value"),
                step,
            },
            period,
            printed_share: None,
        };
        let rules = xf::Rules::new(xf::Convention::Closed)
            .with("sites_to_region", xf::Construction::RoundedOnce);
        let mut outcomes = [0_usize; 2];
        for trial in 0..400 {
            let parts: Vec<Slot> = (0..2 + next(4))
                .map(|k| {
                    let step = steps[next(3) as usize];
                    slot(format!("T!B{k}"), i128::from(next(100_000)), step)
                })
                .collect();
            let mut sum = Rat::from_integer(0);
            for p in &parts {
                if let Operand::Number { value, .. } = p.operand {
                    sum = sum.checked_add(value).expect("sum");
                }
            }
            let step = steps[next(3) as usize];
            let q = sum.checked_div(step).expect("units");
            let total = slot(
                "T!Z1".to_string(),
                q.num().div_euclid(q.den()) + i128::from(next(7)) - 3,
                step,
            );
            let refs: Vec<&Slot> = parts.iter().collect();
            let closed = sum_holds(&refs, &total).expect("no overflow");
            let identity = xf::Identity {
                id: format!("sum{trial}"),
                family: "sites_to_region".to_string(),
                closure: xf::Closure::Definitional,
                kind: xf::IdentityKind::Sum {
                    total: total.locator.clone(),
                    lines: parts.iter().map(|p| p.locator.clone()).collect(),
                    signs: vec![1; parts.len()],
                },
            };
            let cells: Vec<xf::Cell> = parts.iter().chain([&total]).filter_map(xf_cell).collect();
            let solved = xf::solve(&cells, &[identity], &rules).expect("solves");
            let [c] = solved.as_slice() else {
                panic!("trial {trial}: one component, got {}", solved.len());
            };
            let kernel = match &c.outcome {
                xf::Outcome::Feasible { .. } => true,
                xf::Outcome::Infeasible { .. } => false,
                o => panic!("trial {trial}: {o:?}"),
            };
            assert_eq!(closed, kernel, "trial {trial}");
            outcomes[usize::from(closed)] += 1;
        }
        assert!(outcomes.iter().all(|&n| n >= 50), "{outcomes:?}");
    }

    /// Small exhaustive fractional-extrema oracle, independent of the production affine-bound selection.
    fn corner_oracle(region: Interval, parts: &[(Interval, Interval)]) -> bool {
        assert!(!parts.is_empty() && parts.len() <= 8);
        let a: Vec<_> = parts.iter().map(|((a, _), _)| *a).collect();
        let b: Vec<_> = parts.iter().map(|((_, b), _)| *b).collect();
        let mut lower_reached = false;
        let mut upper_reached = false;
        for mask in 0..(1_usize << parts.len()) {
            let weights: Vec<_> = parts
                .iter()
                .enumerate()
                .map(|(i, (_, (c, d)))| if mask & (1 << i) == 0 { *c } else { *d })
                .collect();
            lower_reached |= at_most(mean(&a, &weights).unwrap(), region.1).unwrap();
            upper_reached |= at_most(region.0, mean(&b, &weights).unwrap()).unwrap();
        }
        lower_reached && upper_reached
    }

    /// [`weighted_mean_holds`] against an exhaustive corner oracle and the separable predicate on random boxes
    /// of mixed steps. The kernel's McCormick relaxation must never refute a positive exact decision.
    #[test]
    fn test_exact_weighted_mean_matches_the_separable_test_and_the_kernel() {
        let steps = [
            Rat::from_integer(1),
            Rat::new(1, 10).expect("step"),
            Rat::new(1, 100).expect("step"),
        ];
        let mut seed = 0x0517_u64;
        let mut next = |n: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        let rat =
            |units: i128, step: Rat| Rat::from_integer(units).checked_mul(step).expect("value");
        let half = |step: Rat| step.checked_div(Rat::from_integer(2)).expect("half");
        let span = |v: Rat, step: Rat| {
            (
                v.checked_sub(half(step)).expect("lo"),
                v.checked_add(half(step)).expect("hi"),
            )
        };
        let mut outcomes = [0_usize; 2];
        let mut refuted = 0;
        for trial in 0..600 {
            let n = 1 + next(5) as usize;
            let mut cells = Vec::new();
            let mut parts = Vec::new();
            let mut printed = Vec::new();
            for k in 0..n {
                let (rs, ws) = (steps[next(3) as usize], steps[next(3) as usize]);
                let (r, w) = (
                    rat(i128::from(1_000 + next(100_000)), rs),
                    rat(i128::from(1 + next(5_000)), ws),
                );
                printed.push((r, w));
                parts.push((span(r, rs), span(w, ws)));
                for (name, value, step) in [(format!("r{k}"), r, rs), (format!("w{k}"), w, ws)] {
                    cells.push(xf::Cell {
                        locator: name,
                        sha256: String::new(),
                        token: String::new(),
                        printed: value,
                        step,
                        scope: xf::Scope::Visible,
                    });
                }
            }
            let rates: Vec<Rat> = printed.iter().map(|(r, _)| *r).collect();
            let weights: Vec<Rat> = printed.iter().map(|(_, w)| *w).collect();
            let m = mean(&rates, &weights).expect("mean");
            let rs = steps[next(3) as usize];
            let q = m.checked_div(rs).expect("units");
            let region_value = rat(q.num().div_euclid(q.den()) + i128::from(next(61)) - 30, rs);
            let region = span(region_value, rs);
            cells.push(xf::Cell {
                locator: "R".to_string(),
                sha256: String::new(),
                token: String::new(),
                printed: region_value,
                step: rs,
                scope: xf::Scope::Visible,
            });
            let holds = weighted_mean_holds(region, &parts).expect("decided");
            assert_eq!(
                holds,
                corner_oracle(region, &parts),
                "corner oracle trial {trial}"
            );
            // The separable test.
            let (e, f) = region;
            let mut up = Rat::from_integer(0);
            let mut dn = Rat::from_integer(0);
            for ((a, b), (c, d)) in &parts {
                let term = |x: Rat, w: Rat| x.checked_mul(w).expect("term");
                let (bu, au) = (
                    b.checked_sub(e).expect("b-e"),
                    a.checked_sub(f).expect("a-f"),
                );
                let hi = [term(bu, *c), term(bu, *d)];
                let lo = [term(au, *c), term(au, *d)];
                let pick = |xs: [Rat; 2], greater: bool| {
                    let ord = xs[0].checked_cmp(xs[1]).expect("cmp");
                    if (ord == std::cmp::Ordering::Less) == greater {
                        xs[1]
                    } else {
                        xs[0]
                    }
                };
                up = up.checked_add(pick(hi, true)).expect("up");
                dn = dn.checked_add(pick(lo, false)).expect("dn");
            }
            assert_eq!(holds, up.signum() >= 0 && dn.signum() <= 0, "trial {trial}");
            // The kernel's relaxation.
            let identity = xf::Identity {
                id: format!("wm{trial}"),
                family: Family::SoldWeightedRate.as_str().to_string(),
                closure: xf::Closure::MayUseUnpublished,
                kind: xf::IdentityKind::WeightedMean {
                    rate: "R".to_string(),
                    parts: (0..n).map(|k| (format!("r{k}"), format!("w{k}"))).collect(),
                },
            };
            let solved = xf::solve(&cells, &[identity], &xf::Rules::new(xf::Convention::Closed))
                .expect("solves");
            let [c] = solved.as_slice() else {
                panic!("trial {trial}: one component, got {}", solved.len());
            };
            if matches!(c.outcome, xf::Outcome::Infeasible { .. }) {
                refuted += 1;
                assert!(
                    !holds,
                    "trial {trial}: the relaxation refutes a feasible system"
                );
            }
            outcomes[usize::from(holds)] += 1;
        }
        assert!(outcomes.iter().all(|&k| k >= 50), "{outcomes:?}");
        assert!(refuted >= 50, "the kernel refuted only {refuted}");
    }

    #[test]
    fn weighted_mean_handles_large_regions_without_a_member_ceiling() {
        let r = Rat::from_integer;
        for n in [17, 128, 10_000] {
            let parts = vec![((r(1), r(3)), (r(1), r(2))); n];
            assert_eq!(
                weighted_mean_holds((r(2), r(2)), &parts),
                Some(true),
                "n={n}"
            );
            assert_eq!(
                weighted_mean_holds((r(4), r(5)), &parts),
                Some(false),
                "n={n}"
            );
            assert_eq!(
                weighted_mean_holds((r(-1), r(0)), &parts),
                Some(false),
                "n={n}"
            );
        }
    }

    #[test]
    fn weighted_mean_checks_interpolated_signed_and_boundary_witnesses() {
        let r = Rat::from_integer;
        // Neither selected corner reaches 5: their means are 5/2 and 15/2. Interior weights (2, 2) do.
        let parts = [((r(0), r(0)), (r(1), r(3))), ((r(10), r(10)), (r(1), r(3)))];
        for region in [
            (r(5), r(5)),
            (Rat::new(5, 2).unwrap(), Rat::new(5, 2).unwrap()),
        ] {
            assert!(corner_oracle(region, &parts));
            assert_eq!(weighted_mean_holds(region, &parts), Some(true));
        }
        assert_eq!(weighted_mean_holds((r(8), r(9)), &parts), Some(false));
        let signed = [((r(-4), r(-3)), (r(1), r(3))), ((r(5), r(6)), (r(1), r(3)))];
        assert_eq!(weighted_mean_holds((r(1), r(1)), &signed), Some(true));
        let fixed = [
            ((r(-10), r(-10)), (r(2), r(2))),
            ((r(10), r(10)), (r(2), r(2))),
        ];
        assert_eq!(weighted_mean_holds((r(0), r(0)), &fixed), Some(true));
        assert_eq!(weighted_mean_holds((r(1), r(1)), &fixed), Some(false));
        // A single member needs interpolation between rate bounds, including exact closed endpoints.
        for value in [-3, -2, -1] {
            assert_eq!(
                weighted_mean_holds((r(value), r(value)), &[((r(-3), r(-1)), (r(2), r(5)))]),
                Some(true)
            );
        }
    }

    #[test]
    fn weighted_mean_invalid_boxes_and_overflow_never_claim_a_verdict() {
        let r = Rat::from_integer;
        assert_eq!(weighted_mean_holds((r(0), r(1)), &[]), None);
        assert_eq!(
            weighted_mean_holds((r(2), r(1)), &[((r(0), r(1)), (r(1), r(2)))]),
            None
        );
        for part in [
            ((r(2), r(1)), (r(1), r(2))),
            ((r(0), r(1)), (r(2), r(1))),
            ((r(0), r(1)), (r(0), r(1))),
            ((r(0), r(1)), (r(-2), r(-1))),
            ((r(0), r(1)), (r(0), r(0))),
        ] {
            assert_eq!(weighted_mean_holds((r(0), r(1)), &[part]), None);
        }
        assert_eq!(
            weighted_mean_holds(
                (r(0), r(1)),
                &[((r(i128::MAX), r(i128::MAX)), (r(2), r(2)))]
            ),
            None
        );
    }

    #[test]
    fn sold_weighted_ledger_decides_more_than_sixteen_members() {
        let period = Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        };
        let slot = |name: String, value| Slot {
            locator: name,
            operand: Operand::Number {
                value: Rat::from_integer(value),
                step: Rat::from_integer(1),
            },
            period,
            printed_share: None,
        };
        let members: Vec<_> = (0..64)
            .map(|i| (slot(format!("T!R{i}"), 100), slot(format!("T!W{i}"), 10)))
            .collect();
        let refs: Vec<_> = members
            .iter()
            .map(|(rate, weight)| (rate, weight))
            .collect();
        assert_eq!(
            decide_weighted(&refs, &slot("T!Total".into(), 100)),
            (State::Consistent, false)
        );
        assert_eq!(
            decide_weighted(&refs, &slot("T!Total".into(), 110)),
            (State::Unresolved(Need::CostTotals), false)
        );
    }
}
