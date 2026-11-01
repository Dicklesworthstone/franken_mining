//! bd-4wvl.2 (BD-340.S2): the exact kernel `fmine_numerics::exact_feas` against `aisc.rs`, the bespoke footing
//! and per-ounce checks written earlier, used as an independent oracle: every instance of the synthetic AISC
//! table, and 10⁴ seeded (n, k, q, r, s) cases against the corner rule.
//!
//! Since BD-331.S6, `aisc.rs` itself decides on this kernel, so the synthetic-table comparison checks the
//! ledger's posing against this file's posing, no longer against independent arithmetic. The corner rule,
//! copied here as [`oracle::corner_rate_holds`], stays an independent oracle for the 10⁴ quadruples.
//!
//! Each instance is posed as `aisc.rs` poses it. A footing is its printed total and summands under
//! `ComponentSum`. A per-ounce rate is the printed numerator `n` with `k` half-widths (`aisc.rs`'s N ± k/2, which
//! `prop_component_sum_numerator_matches_k_half_widths` in the kernel shows equal to a component-sum numerator
//! over its lines), ounces `q` and rate `r` at step 1, and `rate = 1000·s·N/Q` with the registered share `s`
//! where the stated share rule applies (`s = 1` otherwise). Instances `aisc.rs` settles before any arithmetic
//! (refused or not published) are not solver questions; they are counted, not compared.

// In a module under `cfg(test)` so the helpers get clippy's test allowances (unwrap, expect, panic).
#[cfg(test)]
mod oracle {
    use fmine_core::{PhiloxStream, RngKey};
    use fmine_numerics::exact_feas::{
        self as xf, Cell, Certificate, Closure, Construction, Convention, Identity, IdentityKind,
        ImpliedError, Interval, Outcome, Rat, Rules, Scope, implied_interval, solve,
    };
    use fmine_public::aisc::{
        Calc, IdentityFamily, IdentityInstance, IdentityState, OperandRole, OperandValue, RateMiss,
        feasible_share, implied_ounces, rate_holds, rate_range, reconstruction_holds,
    };
    use fmine_public::run_synthetic_assurance;
    use fmine_public::synthetic::SYNTHETIC_SHARES;

    /// The closed form against the kernel's share interval (bd-4wvl.4 part 3), which derives it from each
    /// factor's rounding box: r and q at step 1, N = n at step k (k summed lines), 1/1000 the constant.
    #[test]
    fn test_feasible_share_matches_kernel_share_interval() {
        use xf::discover::{Factor, share_interval};
        let at = |v: i64, step: i64| Factor {
            printed: Rat::from_integer(i128::from(v)),
            step: Rat::from_integer(i128::from(step)),
        };
        let mut compared = 0;
        for n in 1..=40 {
            for k in 0..=3 {
                for q in 1..=20 {
                    for r in [1, 7, 950, 1501, 2736] {
                        let Some(((ln, ld), (hn, hd))) = feasible_share(n, k, q, r) else {
                            continue;
                        };
                        let s = share_interval(
                            Rat::new(1, 1000).expect("1/1000"),
                            &[at(r, 1), at(q, 1)],
                            &[at(n, k)],
                            xf::Convention::Closed,
                        )
                        .expect("kernel interval");
                        let ends = (
                            s.lo.map(|e| (e.at, e.strict)),
                            s.hi.map(|e| (e.at, e.strict)),
                        );
                        let want = (
                            Some((Rat::new(ln, ld).expect("lo"), false)),
                            Some((Rat::new(hn, hd).expect("hi"), false)),
                        );
                        assert_eq!(ends, want, "n {n} k {k} q {q} r {r}");
                        compared += 1;
                    }
                }
            }
        }
        // 40·4·20·5 = 16,000 points, less (n, k) = (1, 2) and (1, 3) at 100 each, which have 2n − k < 1.
        assert_eq!(compared, 15_800, "every grid point with 2n − k ≥ 1");
    }

    /// [`rate_holds`] at a share, decided.
    fn holds_at(n: i64, k: i64, q: i64, r: i64, share: (i64, i64)) -> bool {
        rate_holds(n, k, q, r, share).expect("the kernel decides")
    }

    /// [`rate_holds`] at 100%, decided.
    fn holds(n: i64, k: i64, q: i64, r: i64) -> bool {
        holds_at(n, k, q, r, (1, 1))
    }

    #[test]
    fn reconstruction_known_answers() {
        // N = 30 ± 1, Q = 20 ± ½, printed 2000 needs N ∈ [38.990…, 41.010…]: adding 10 ± ½ gives
        // [38.5, 41.5] and closes it; adding 5 ± ½ gives [33.5, 36.5] and does not.
        let miss = RateMiss {
            n: 30,
            k: 2,
            q: 20,
            r: 2000,
        };
        assert_eq!(reconstruction_holds(miss, 10), Ok(true));
        assert_eq!(reconstruction_holds(miss, 5), Ok(false));
        assert!(!holds(30, 2, 20, 2000));
    }

    /// BD-331.S6: the port changed no state. At cd423b9 the two kernels ran side by side on all 55 synthetic
    /// instances with no difference; the corner kernel is gone, and its root at 9aab4a6
    /// (`fmine assure public --synthetic`), b50f7779…, pinned the result. bd-jg43.3 then changed the inputs on
    /// purpose (column G styled, column N added, frame v5), so this is now a regression pin of that commit's
    /// output: the five moved and five new states are pinned one by one by the synthetic tests in `lib.rs`.
    /// Frame v6 (bd-prmn.4) appends the Mine Stats ledger section, empty here: at v5 without it the root is
    /// still 54f1201b…, checked when the pin moved. Frame v7 (site-sheet family) changes only the version here:
    /// at v6 the root is still fd7864f9…, checked when the pin moved. BD-333's frame v8 adds the
    /// empty waterfall digest here; the v7 regression root was
    /// dfb17a08c70d5307ece489544334cea72086234182fe9d3d0580e3a0d209f5e5. These are regression
    /// values, not independent numerical oracles; the unchanged 60 states remain separately checked.
    #[test]
    fn test_port_keeps_synthetic_root() {
        let run = run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(run.instances.len() + run.hidden_scope.instances.len(), 60);
        assert_eq!(
            run.mine_ledger,
            fmine_public::mine_ledger::MineLedger::default()
        );
        assert!(run.waterfall.pairs.is_empty());
        assert_eq!(
            run.ledger_anchor.state_root.to_hex(),
            "eb8bd4e4ae85d74b07b14e29e9c05a198b90491ea821e269bb72127972fe2ff0"
        );
    }

    /// bd-prmn.8: the one-cell check and its implied ounces on hand-computed columns.
    #[test]
    fn one_cell_implied_ounces_known_answers() {
        // n = 100, q = 50, r = 2000: 1000·100/50 = 2000 exactly, inside.
        assert!(holds(100, 1, 50, 2000));
        // The implied ounces at 100%: [1000·199/4001, 1000·201/3999] = [49.7375…, 50.2625…], which holds 50.
        assert_eq!(
            implied_ounces(100, 2000, 1, 1),
            ("49.737".to_string(), "50.263".to_string())
        );
        // r = 2100: [1000·199/4201, 1000·201/4199] = [47.3696…, 47.8685…] misses [49.5, 50.5], so it fails.
        assert!(!holds(100, 1, 50, 2100));
        assert_eq!(
            implied_ounces(100, 2100, 1, 1),
            ("47.369".to_string(), "47.869".to_string())
        );
        // At s = 3/4 the implied ounces scale by 3/4: [1000·3·199/(4·4201), 1000·3·201/(4·4199)]
        // = [35.5272…, 35.9014…]; q = 36 lies in [35.5, 36.5] and meets it, so the scaled check passes.
        assert_eq!(
            implied_ounces(100, 2100, 3, 4),
            ("35.527".to_string(), "35.902".to_string())
        );
        assert!(holds_at(100, 1, 36, 2100, (3, 4)));
        assert!(!holds_at(100, 1, 34, 2100, (3, 4)));
    }

    /// The corner test and the feasible interval are the same rule: s fits iff lo ≤ s ≤ hi, end points included.
    #[test]
    fn test_corner_test_agrees_with_feasible_interval() {
        let shares = [
            (21, 25),
            (123, 200),
            (3, 5),
            (4, 5),
            (897, 1000),
            (3, 4),
            (1, 1),
        ];
        let mut fits = 0;
        for n in [1, 2, 7, 117, 400] {
            for q in [1, 3, 37, 150] {
                for r in [1, 500, 2000, 2736, 9000] {
                    let Some(((ln, ld), (hn, hd))) = feasible_share(n, 1, q, r) else {
                        panic!("positive inputs have an interval")
                    };
                    let (lo, hi) = (Rat::new(ln, ld).expect("lo"), Rat::new(hn, hd).expect("hi"));
                    for (a, b) in shares {
                        let s = Rat::new(i128::from(a), i128::from(b)).expect("share");
                        let inside = s.checked_cmp(lo).expect("cmp").is_ge()
                            && s.checked_cmp(hi).expect("cmp").is_le();
                        assert_eq!(
                            holds_at(n, 1, q, r, (a, b)),
                            inside,
                            "n={n} q={q} r={r} s={a}/{b}"
                        );
                        fits += usize::from(inside);
                    }
                }
            }
        }
        // Both outcomes occur, so the agreement is not vacuous.
        assert!(fits > 0 && fits < 5 * 4 * 5 * shares.len(), "{fits}");
    }

    fn int(n: i64) -> Rat {
        Rat::from_integer(i128::from(n))
    }

    fn cell(locator: &str, printed: i64, step: i64) -> Cell {
        Cell {
            locator: locator.to_string(),
            sha256: String::new(),
            token: printed.to_string(),
            printed: int(printed),
            step: int(step),
            scope: Scope::Visible,
        }
    }

    fn only(components: Vec<fmine_numerics::exact_feas::Component>) -> Outcome {
        assert_eq!(components.len(), 1, "{components:?}");
        components.into_iter().next().expect("one").outcome
    }

    /// The ratio N, Q, R as `aisc.rs` poses a per-ounce rate.
    fn rate_problem(n: i64, k: i64, q: i64, r: i64, factor: Rat) -> (Vec<Cell>, Vec<Identity>) {
        (
            vec![cell("N", n, k), cell("Q", q, 1), cell("R", r, 1)],
            vec![Identity {
                id: "rate".to_string(),
                family: "per_ounce".to_string(),
                closure: Closure::MayUseUnpublished,
                kind: IdentityKind::Ratio {
                    rate: "R".to_string(),
                    num: "N".to_string(),
                    den: "Q".to_string(),
                    factor,
                },
            }],
        )
    }

    fn rate_outcome(n: i64, k: i64, q: i64, r: i64, factor: Rat) -> Outcome {
        let (cells, ids) = rate_problem(n, k, q, r, factor);
        only(solve(&cells, &ids, &Rules::new(Convention::Closed)).expect("solves"))
    }

    /// A footing as `aisc.rs` poses it: the printed total and its summands (a placeholder dash reads as 0, as
    /// `aisc.rs` reads it), under `ComponentSum`.
    fn footing_outcome(inst: &IdentityInstance) -> Outcome {
        let value = |v: &OperandValue| match v {
            OperandValue::Int(x) => *x,
            OperandValue::Token(_) => 0,
            other => panic!("{}: an arithmetic footing read {other:?}", inst.cell),
        };
        let mut cells = Vec::new();
        let mut total = None;
        let mut lines = Vec::new();
        for o in &inst.operands {
            match o.role {
                OperandRole::Total => {
                    total = Some(o.cell.clone());
                    cells.push(cell(&o.cell, value(&o.value), 1));
                }
                OperandRole::Line => {
                    lines.push(o.cell.clone());
                    cells.push(cell(&o.cell, value(&o.value), 1));
                }
                _ => {}
            }
        }
        let ids = vec![Identity {
            id: "footing".to_string(),
            family: "footing".to_string(),
            closure: Closure::Definitional,
            kind: IdentityKind::Sum {
                total: total.expect("a footing has a total"),
                signs: vec![1; lines.len()],
                lines,
            },
        }];
        let rules = Rules::new(Convention::Closed).with("footing", Construction::ComponentSum);
        only(solve(&cells, &ids, &rules).expect("solves"))
    }

    /// The per-ounce factor: 1000, times the registered share where the stated share rule applies (cost of sales
    /// per ounce with a non-zero non-controlling-interest line). `None` when the rule applies and no share is
    /// registered: `aisc.rs` refuses that instance before any arithmetic.
    fn rate_factor(inst: &IdentityInstance) -> Option<Rat> {
        let nci = inst.operands.iter().any(|o| {
            o.role == OperandRole::NciLine && matches!(o.value, OperandValue::Int(x) if x != 0)
        });
        if inst.family != IdentityFamily::CosPerOunce || !nci {
            return Some(int(1000));
        }
        SYNTHETIC_SHARES
            .iter()
            .find(|(subject, _, _)| *subject == inst.subject)
            .map(|&(_, num, den)| {
                Rat::new(1000 * i128::from(num), i128::from(den)).expect("share factor")
            })
    }

    /// The kernel's state code for an instance `aisc.rs` decided by arithmetic, or `None` for one it settled first.
    fn solver_code(inst: &IdentityInstance) -> Option<String> {
        match inst.calc {
            Calc::None => None,
            Calc::Footing { .. } => Some(
                match footing_outcome(inst) {
                    Outcome::Feasible { .. } => "i",
                    Outcome::Infeasible {
                        certificate: Certificate::Residual { .. },
                    } => "iii",
                    other => panic!("{} footing: {other:?}", inst.cell),
                }
                .to_string(),
            ),
            Calc::Rate(m) => {
                let factor = rate_factor(inst)?;
                Some(
                    match rate_outcome(m.n, m.k, m.q, m.r, factor) {
                        Outcome::Feasible { .. } => "i",
                        Outcome::Infeasible { .. } => match &inst.state {
                            // The registered component aisc.rs cites, added to the numerator with one more half-width:
                            // the kernel re-decides whether it closes.
                            IdentityState::Explained { component, .. } => {
                                match rate_outcome(m.n + component.value, m.k + 1, m.q, m.r, factor)
                                {
                                    Outcome::Feasible { .. } => "ii",
                                    other => {
                                        panic!(
                                            "{}: the component does not close: {other:?}",
                                            inst.cell
                                        )
                                    }
                                }
                            }
                            _ => "iv",
                        },
                        other => panic!("{} rate: {other:?}", inst.cell),
                    }
                    .to_string(),
                )
            }
        }
    }

    #[test]
    fn test_synthetic_aisc_table_states_match_aisc_rs() {
        let run = run_synthetic_assurance(&[]).expect("synthetic run");
        let in_b_to_h = |inst: &&IdentityInstance| {
            matches!(
                inst.column.as_str(),
                "B" | "C" | "D" | "E" | "F" | "G" | "H"
            )
        };
        assert_eq!(run.instances.iter().filter(in_b_to_h).count(), 35);
        let (mut decided, mut settled_first) = (0, 0);
        // The 35 visible instances of columns B–H (the hidden sheet's column B is counted with the rest).
        let (mut decided_b_to_h, mut settled_b_to_h) = (0, 0);
        let mut codes: Vec<String> = Vec::new();
        let visible = run.instances.iter().map(|i| (i, true));
        for (inst, shown) in visible.chain(run.hidden_scope.instances.iter().map(|i| (i, false))) {
            let code = inst.state.code();
            let counted = shown && in_b_to_h(&inst);
            match solver_code(inst) {
                Some(kernel) => {
                    assert_eq!(
                        kernel, code,
                        "{}!{} {:?} {:?}",
                        inst.sheet, inst.cell, inst.family, inst.state
                    );
                    decided += 1;
                    decided_b_to_h += usize::from(counted);
                    codes.push(format!("{}!{}:{code}", inst.sheet, inst.cell));
                }
                None => {
                    assert!(
                        matches!(code, "v" | "np"),
                        "{}!{} settled first as {code}",
                        inst.sheet,
                        inst.cell
                    );
                    settled_first += 1;
                    settled_b_to_h += usize::from(counted);
                }
            }
        }
        assert_eq!(decided_b_to_h + settled_b_to_h, 35);
        // The seeded answers the kernel reproduces: C8 (iii), D16 (iv), H16 (ii, reconstructed), K14 (i at the
        // stated share), L14 (iv at the stated share), M16 (ii, stated), and the hidden sheet's B8 (iii).
        for seeded in [
            "Q1 2030!C8:iii",
            "Q1 2030!D16:iv",
            "Q1 2030!H16:ii",
            "Q1 2030!K14:i",
            "Q1 2030!L14:iv",
            "Q1 2030!M16:ii",
            "Q4 2029!B8:iii",
        ] {
            assert!(
                codes.iter().any(|c| c == seeded),
                "{seeded} not decided: {codes:?}"
            );
        }
        println!(
            "{{\"test\":\"synthetic_states\",\"visible\":{},\"hidden\":{},\"decided_by_kernel\":{decided},\
         \"settled_before_arithmetic\":{settled_first},\"b_to_h_decided\":{decided_b_to_h},\
         \"b_to_h_settled_first\":{settled_b_to_h},\"disagree\":0}}",
            run.instances.len(),
            run.hidden_scope.instances.len()
        );
        assert_eq!(
            decided + settled_first,
            run.instances.len() + run.hidden_scope.instances.len()
        );
    }

    #[test]
    fn test_component_sum_footing_matches_aisc_synthetic() {
        let run = run_synthetic_assurance(&[]).expect("synthetic run");
        let mut residuals = Vec::new();
        for inst in run.instances.iter().chain(&run.hidden_scope.instances) {
            let Calc::Footing {
                printed,
                component_sum,
                ..
            } = inst.calc
            else {
                continue;
            };
            match (footing_outcome(inst), &inst.state) {
                (Outcome::Feasible { .. }, IdentityState::Consistent { .. }) => {
                    assert_eq!(printed, component_sum, "{}", inst.cell);
                }
                (
                    Outcome::Infeasible {
                        certificate: Certificate::Residual { residual, .. },
                    },
                    IdentityState::IssuerDiscrepancy(c),
                ) => {
                    assert_eq!(residual, int(c.residual), "{}", inst.cell);
                    residuals.push((inst.sheet.clone(), inst.cell.clone(), c.residual));
                }
                (kernel, state) => panic!("{}: kernel {kernel:?}, aisc.rs {state:?}", inst.cell),
            }
        }
        // C8: 350 printed against 300 − 50 + 0 = 250. The hidden sheet's B column: TCC 401 against 400.
        assert!(
            residuals.contains(&("Q1 2030".to_string(), "C8".to_string(), 100)),
            "{residuals:?}"
        );
        assert!(
            residuals.contains(&("Q4 2029".to_string(), "B8".to_string(), 1)),
            "{residuals:?}"
        );
    }

    /// D16 (iv): r 2400 against N = 200 over k = 5 lines and Q = 100, where `aisc.rs` allows the rate only in
    /// [1965.174, 2035.176], rounded outward. The kernel's rate range is exactly 1000·[197.5/100.5, 202.5/99.5] =
    /// [395000/201, 405000/199], which rounds outward to the same text. It also states the miss from the other
    /// side, exactly: the numerator would need
    /// N ∈ [2399.5·99.5, 2400.5·100.5]/1000 = [955001/4000, 965001/4000] against its [197.5, 202.5], and the ounces
    /// Q ∈ [1000·197.5/2400.5, 1000·202.5/2399.5] = [395000/4801, 405000/4799] against [99.5, 100.5].
    #[test]
    fn test_d16_miss_as_implied_intervals() {
        let run = run_synthetic_assurance(&[]).expect("synthetic run");
        let d16 = run
            .instances
            .iter()
            .find(|i| i.cell == "D16" && i.family == IdentityFamily::AiscPerOunce)
            .expect("D16");
        let Calc::Rate(m) = d16.calc else {
            panic!("D16 is a rate: {:?}", d16.calc);
        };
        assert_eq!((m.n, m.k, m.q, m.r), (200, 5, 100, 2400));
        assert!(d16.state.detail().contains("[1965.174, 2035.176]"));
        let (cells, ids) = rate_problem(m.n, m.k, m.q, m.r, int(1000));
        let rules = Rules::new(Convention::Closed);
        let exact = |n: i128, d: i128| Rat::new(n, d).expect("rational");
        let closed = |lo, hi| Interval {
            lo,
            hi,
            lo_strict: false,
            hi_strict: false,
        };
        assert_eq!(
            implied_interval(&cells, &ids, &rules, "N", true),
            Ok(closed(exact(955_001, 4000), exact(965_001, 4000)))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &rules, "Q", true),
            Ok(closed(exact(395_000, 4801), exact(405_000, 4799)))
        );
        assert_eq!(
            implied_interval(&cells, &ids, &rules, "Q", false),
            Err(ImpliedError::Infeasible)
        );
        let range = implied_interval(&cells, &ids, &rules, "R", true).expect("the rate range");
        assert_eq!(range, closed(exact(395_000, 201), exact(405_000, 199)));
        // Outward to three decimals, in integers: ⌊1000·lo⌋ and ⌈1000·hi⌉ thousandths.
        let thousandths = |x: Rat, up: bool| {
            let (num, den) = (x.num() * 1000, x.den());
            if up {
                -(-num).div_euclid(den)
            } else {
                num.div_euclid(den)
            }
        };
        let text = |t: i128| format!("{}.{:03}", t / 1000, t % 1000);
        let shown = format!(
            "[{}, {}]",
            text(thousandths(range.lo, false)),
            text(thousandths(range.hi, true))
        );
        assert_eq!(shown, "[1965.174, 2035.176]");
        assert!(d16.state.detail().contains(&shown));
        // With its own box [2399.5, 2400.5] the rate has no value.
        assert_eq!(
            implied_interval(&cells, &ids, &rules, "R", false),
            Err(ImpliedError::Infeasible)
        );
    }

    /// The four corners of 1000·s·N/Q over N ∈ [n − k/2, n + k/2] and Q ∈ [q − ½, q + ½], s = a/b:
    /// 1000·a·(2n ± k)/(b·(2q ± 1)), as (numerator, denominator).
    fn corners(n: i64, k: i64, q: i64, (a, b): (i64, i64)) -> [(i128, i128); 4] {
        let (n2, k, q2) = (2 * i128::from(n), i128::from(k), 2 * i128::from(q));
        let (a, b) = (i128::from(a), i128::from(b));
        [
            (1000 * a * (n2 - k), b * (q2 - 1)),
            (1000 * a * (n2 - k), b * (q2 + 1)),
            (1000 * a * (n2 + k), b * (q2 - 1)),
            (1000 * a * (n2 + k), b * (q2 + 1)),
        ]
    }

    /// The closed-form corner rule `aisc.rs` used before BD-331.S6 moved the ledger onto the kernel, kept here as
    /// an independent oracle: whether some N ∈ [n − k/2, n + k/2] and Q ∈ [q − ½, q + ½] give 1000·s·N/Q ∈
    /// [r − ½, r + ½]. 1000·s·N/Q is monotone along each edge of the box (Q never crosses zero for |q| ≥ 1), so
    /// its range is spanned by the four [`corners`], compared by exact cross-multiplication.
    fn corner_rate_holds(n: i64, k: i64, q: i64, r: i64, share: (i64, i64)) -> bool {
        let r2 = 2 * i128::from(r);
        // num/den against c/2, with den's sign folded in.
        let cmp = |num: i128, den: i128, c: i128| {
            let ord = (num * 2).cmp(&(c * den));
            if den < 0 { ord.reverse() } else { ord }
        };
        let corners = corners(n, k, q, share);
        let reaches_low = corners.iter().any(|&(a, b)| cmp(a, b, r2 - 1).is_ge());
        let reaches_high = corners.iter().any(|&(a, b)| cmp(a, b, r2 + 1).is_le());
        reaches_low && reaches_high
    }

    /// The rate's exact range at 100% by the corner rule: the least and greatest corner, both attained.
    fn corner_rate_range(n: i64, k: i64, q: i64) -> Interval {
        let mut values: Vec<Rat> = corners(n, k, q, (1, 1))
            .iter()
            .map(|&(num, den)| Rat::new(num, den).expect("Q never crosses zero"))
            .collect();
        values.sort_by(|x, y| x.checked_cmp(*y).expect("comparable"));
        Interval {
            lo: values[0],
            hi: values[3],
            lo_strict: false,
            hi_strict: false,
        }
    }

    /// The linearised ratio against [`corner_rate_holds`] on 10⁴ seeded cases: n in [−3000, 3000], k in [1, 9],
    /// |q| in [1, 400] (one in eight negative), a share s among five (100% and four registered-style shares), and
    /// r within about twice the rate range's half-width of 1000·s·n/q. Also the ledger's own entry points:
    /// `aisc::rate_holds` at the share, and `aisc::rate_range` (the (iv) range) at 100%, exactly.
    #[test]
    fn prop_ratio_linearisation_matches_corner_rule() {
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 340,
            partition: 3,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |n: u64| (rng.next_u64() % n) as i64;
        let shares = [(1, 1), (3, 4), (123, 200), (21, 25), (897, 1000)];
        let quadruples = 10_000;
        let (mut holds, mut fails) = (0_usize, 0_usize);
        for case in 0..quadruples {
            let n = next(6001) - 3000;
            let k = 1 + next(9);
            let magnitude = 1 + next(400);
            let q = if next(8) == 0 { -magnitude } else { magnitude };
            let share = shares[next(5) as usize];
            let factor = Rat::new(1000 * i128::from(share.0), i128::from(share.1)).expect("factor");
            let spread = 2 + (500 * k) / magnitude + (500 * n.abs()) / (magnitude * magnitude);
            let centre = (1000 * share.0 * n).div_euclid(share.1 * q);
            let r = centre + next((4 * spread + 1) as u64) - 2 * spread;
            let expected = corner_rate_holds(n, k, q, r, share);
            assert_eq!(
                rate_holds(n, k, q, r, share),
                Ok(expected),
                "case {case}: the ledger's entry point at {share:?}"
            );
            if share == (1, 1) {
                assert_eq!(
                    rate_range(n, k, q),
                    Ok(corner_rate_range(n, k, q)),
                    "case {case}: the (iv) range"
                );
            }
            let kernel = match rate_outcome(n, k, q, r, factor) {
                Outcome::Feasible { .. } => true,
                Outcome::Infeasible { .. } => false,
                other => panic!("case {case} ({n}, {k}, {q}, {r}): {other:?}"),
            };
            assert_eq!(
                kernel, expected,
                "case {case}: (n, k, q, r, s) = ({n}, {k}, {q}, {r}, {share:?})"
            );
            // The same answer by the other path: the rate's range by Charnes–Cooper, cut by r's own box.
            let (cells, ids) = rate_problem(n, k, q, r, factor);
            let rules = Rules::new(Convention::Closed);
            match implied_interval(&cells, &ids, &rules, "R", false) {
                Ok(_) => assert!(expected, "case {case}: a range where aisc.rs has none"),
                Err(ImpliedError::Infeasible) => {
                    assert!(!expected, "case {case}: no range where aisc.rs has one")
                }
                Err(other) => panic!("case {case} ({n}, {k}, {q}, {r}): {other:?}"),
            }
            if expected {
                holds += 1;
            } else {
                fails += 1;
            }
        }
        println!(
            "{{\"quadruples\":{quadruples},\"agree\":{quadruples},\"disagree\":0,\"seed\":340,\
         \"holds\":{holds},\"fails\":{fails}}}"
        );
        assert!(holds > quadruples / 10 && fails > quadruples / 10);
    }
}
