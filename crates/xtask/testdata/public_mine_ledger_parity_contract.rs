#[cfg(test)]
mod tests {
    use super::*;

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

    /// [`weighted_mean_holds`] against two other derivations on pseudo-random boxes of mixed steps: the separable
    /// test (max over the weights of Σ (bᵢ − e)·wᵢ ≥ 0 and min of Σ (aᵢ − f)·wᵢ ≤ 0, each term at an end of its
    /// weight's box) must agree exactly, and the kernel's McCormick relaxation, which only refutes, must never
    /// refute what the closed form calls feasible.
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
}
